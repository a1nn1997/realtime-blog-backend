-- Recommendation generation functions

-- Function to clear existing recommendations for specified users (or all users if NULL)
CREATE OR REPLACE FUNCTION global.clear_recommendations(
    target_user_ids UUID[] DEFAULT NULL
) RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    IF target_user_ids IS NULL THEN
        -- Clear all recommendations
        DELETE FROM "global"."recommendations";
    ELSE
        -- Clear recommendations for specific users
        DELETE FROM "global"."recommendations"
        WHERE user_id = ANY(target_user_ids);
    END IF;
    
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- Function to generate content-based recommendations
CREATE OR REPLACE FUNCTION global.generate_content_based_recommendations(
    max_recommendations_per_user INTEGER DEFAULT 10
) RETURNS INTEGER AS $$
DECLARE
    inserted_count INTEGER := 0;
    user_record RECORD;
    expiry_date TIMESTAMPTZ := NOW() + INTERVAL '7 days';
BEGIN
    -- For each user
    FOR user_record IN SELECT id FROM "global"."users"
    LOOP
        -- Insert content-based recommendations based on tag overlap
        WITH user_viewed_tags AS (
            -- Get tags from posts the user has viewed
            SELECT DISTINCT t.id, t.name
            FROM "global"."user_interactions" ui
            JOIN "global"."posts" p ON ui.post_id = p.id
            JOIN "global"."post_tags" pt ON p.id = pt.post_id
            JOIN "global"."tags" t ON pt.tag_id = t.id
            WHERE ui.user_id = user_record
              AND ui.interaction_type = 'view'
        ),
        user_viewed_posts AS (
            -- Get posts the user has already viewed
            SELECT DISTINCT post_id
            FROM "global"."user_interactions"
            WHERE user_id = user_record
              AND interaction_type = 'view'
        ),
        candidate_posts AS (
            -- Find posts with similar tags that user hasn't viewed
            SELECT 
                p.id AS post_id,
                COUNT(DISTINCT pt.tag_id) AS matching_tag_count,
                COUNT(DISTINCT pt.tag_id)::float / 
                    (SELECT COUNT(DISTINCT tag_id) FROM "global"."post_tags" WHERE post_id = p.id)::float AS relevance_score
            FROM "global"."posts" p
            JOIN "global"."post_tags" pt ON p.id = pt.post_id
            JOIN user_viewed_tags uvt ON pt.tag_id = uvt.id
            WHERE p.is_deleted = false
              AND p.is_draft = false
              AND p.id NOT IN (SELECT post_id FROM user_viewed_posts)
            GROUP BY p.id
            HAVING COUNT(DISTINCT pt.tag_id) > 0
            ORDER BY relevance_score DESC, p.created_at DESC
            LIMIT max_recommendations_per_user
        )
        INSERT INTO "global"."recommendations" (
            user_id, post_id, score, recommendation_type, created_at, expires_at
        )
        SELECT 
            user_record, 
            post_id, 
            GREATEST(0.5, LEAST(0.95, relevance_score)), -- Normalize score between 0.5 and 0.95
            'content_based', 
            NOW(), 
            expiry_date
        FROM candidate_posts
        ON CONFLICT (user_id, post_id) DO UPDATE
        SET score = GREATEST("recommendations".score, EXCLUDED.score),
            recommendation_type = 
                CASE 
                    WHEN "recommendations".recommendation_type = 'hybrid' THEN 'hybrid'
                    ELSE EXCLUDED.recommendation_type 
                END,
            expires_at = expiry_date;
    END LOOP;
    
    GET DIAGNOSTICS inserted_count = ROW_COUNT;
    RETURN inserted_count;
END;
$$ LANGUAGE plpgsql;

-- Function to generate collaborative filtering recommendations
CREATE OR REPLACE FUNCTION global.generate_collaborative_recommendations(
    max_recommendations_per_user INTEGER DEFAULT 10
) RETURNS INTEGER AS $$
DECLARE
    inserted_count INTEGER := 0;
    user_record RECORD;
    expiry_date TIMESTAMPTZ := NOW() + INTERVAL '7 days';
BEGIN
    -- For each user
    FOR user_record IN SELECT id FROM "global"."users"
    LOOP
        -- Find similar users (who have interacted with the same posts)
        WITH user_interactions AS (
            -- Get posts this user has interacted with
            SELECT post_id, interaction_type, 
                CASE
                    WHEN interaction_type = 'view' THEN 1
                    WHEN interaction_type = 'like' THEN 3
                    WHEN interaction_type = 'comment' THEN 5
                    ELSE 1
                END AS weight
            FROM "global"."user_interactions"
            WHERE user_id = user_record
        ),
        similar_users AS (
            -- Find users who have interacted with the same posts
            SELECT 
                ui.user_id,
                SUM(u_int.weight) AS similarity_score
            FROM "global"."user_interactions" ui
            JOIN user_interactions u_int ON ui.post_id = u_int.post_id
            WHERE ui.user_id != user_record
            GROUP BY ui.user_id
            HAVING COUNT(DISTINCT ui.post_id) >= 2 -- Require at least 2 common interactions
            ORDER BY similarity_score DESC
            LIMIT 10 -- Top 10 similar users
        ),
        user_viewed_posts AS (
            -- Get posts the user has already viewed
            SELECT DISTINCT post_id
            FROM "global"."user_interactions"
            WHERE user_id = user_record
        ),
        candidate_posts AS (
            -- Get posts that similar users have liked but current user hasn't seen
            SELECT 
                ui.post_id,
                SUM(su.similarity_score) AS weighted_score,
                COUNT(DISTINCT ui.user_id) AS user_count
            FROM "global"."user_interactions" ui
            JOIN similar_users su ON ui.user_id = su.user_id
            WHERE ui.interaction_type IN ('like', 'comment')
              AND ui.post_id NOT IN (SELECT post_id FROM user_viewed_posts)
            GROUP BY ui.post_id
            ORDER BY weighted_score DESC, user_count DESC
            LIMIT max_recommendations_per_user
        )
        INSERT INTO "global"."recommendations" (
            user_id, post_id, score, recommendation_type, created_at, expires_at
        )
        SELECT 
            user_record, 
            post_id, 
            GREATEST(0.5, LEAST(0.9, weighted_score / (SELECT MAX(weighted_score) FROM candidate_posts))), 
            'collaborative', 
            NOW(), 
            expiry_date
        FROM candidate_posts
        WHERE EXISTS (SELECT 1 FROM candidate_posts) -- Only proceed if we have candidates
        ON CONFLICT (user_id, post_id) DO UPDATE
        SET score = GREATEST("recommendations".score, EXCLUDED.score),
            recommendation_type = 
                CASE 
                    WHEN "recommendations".recommendation_type = 'content_based' 
                    OR "recommendations".recommendation_type = 'hybrid' THEN 'hybrid'
                    ELSE EXCLUDED.recommendation_type 
                END,
            expires_at = expiry_date;
    END LOOP;
    
    GET DIAGNOSTICS inserted_count = ROW_COUNT;
    RETURN inserted_count;
END;
$$ LANGUAGE plpgsql;

-- Function to generate popular post recommendations
CREATE OR REPLACE FUNCTION global.generate_popular_recommendations(
    max_recommendations_per_user INTEGER DEFAULT 10
) RETURNS INTEGER AS $$
DECLARE
    inserted_count INTEGER := 0;
    user_record RECORD;
    expiry_date TIMESTAMPTZ := NOW() + INTERVAL '7 days';
BEGIN
    -- For each user
    FOR user_record IN SELECT id FROM "global"."users"
    LOOP
        -- Find popular posts the user hasn't seen
        WITH user_viewed_posts AS (
            -- Get posts the user has already viewed
            SELECT DISTINCT post_id
            FROM "global"."user_interactions"
            WHERE user_id = user_record
        ),
        popular_posts AS (
            -- Get popular posts based on views and likes
            SELECT 
                p.id AS post_id,
                (p.views * 0.6 + p.likes * 0.4) AS popularity_score
            FROM "global"."posts" p
            WHERE p.is_deleted = false
              AND p.is_draft = false
              AND p.id NOT IN (SELECT post_id FROM user_viewed_posts)
            ORDER BY popularity_score DESC, p.created_at DESC
            LIMIT max_recommendations_per_user
        )
        INSERT INTO "global"."recommendations" (
            user_id, post_id, score, recommendation_type, created_at, expires_at
        )
        SELECT 
            user_record, 
            post_id, 
            GREATEST(0.3, LEAST(0.7, popularity_score / 
                               (SELECT MAX(popularity_score) FROM popular_posts))), 
            'popular', 
            NOW(), 
            expiry_date
        FROM popular_posts
        ON CONFLICT (user_id, post_id) DO UPDATE
        SET score = GREATEST("recommendations".score, EXCLUDED.score),
            recommendation_type = 
                CASE 
                    WHEN "recommendations".recommendation_type IN ('content_based', 'collaborative', 'hybrid') 
                    THEN "recommendations".recommendation_type
                    ELSE EXCLUDED.recommendation_type 
                END,
            expires_at = expiry_date;
    END LOOP;
    
    GET DIAGNOSTICS inserted_count = ROW_COUNT;
    RETURN inserted_count;
END;
$$ LANGUAGE plpgsql;

-- Master function to generate all types of recommendations
CREATE OR REPLACE FUNCTION global.populate_recommendations(
    clear_existing BOOLEAN DEFAULT TRUE,
    target_user_ids UUID[] DEFAULT NULL,
    recommendations_per_user INTEGER DEFAULT 20
) RETURNS TEXT AS $$
DECLARE
    content_count INTEGER;
    collab_count INTEGER;
    popular_count INTEGER;
    clear_count INTEGER := 0;
BEGIN
    -- Clear existing recommendations if requested
    IF clear_existing THEN
        clear_count := global.clear_recommendations(target_user_ids);
    END IF;
    
    -- Generate various types of recommendations
    content_count := global.generate_content_based_recommendations(recommendations_per_user);
    collab_count := global.generate_collaborative_recommendations(recommendations_per_user);
    popular_count := global.generate_popular_recommendations(recommendations_per_user);
    
    RETURN format('Cleared %s existing recommendations. Generated %s content-based, %s collaborative, and %s popular recommendations', 
                  clear_count, content_count, collab_count, popular_count);
END;
$$ LANGUAGE plpgsql;

-- Function to generate similar posts (for the similar posts endpoint)
CREATE OR REPLACE FUNCTION global.generate_similar_posts(
    target_post_id BIGINT,
    max_similar_posts INTEGER DEFAULT 10
) RETURNS TABLE (
    post_id BIGINT,
    similarity_score FLOAT
) AS $$
BEGIN
    -- Get tags for the target post
    WITH target_post_tags AS (
        SELECT tag_id
        FROM "global"."post_tags"
        WHERE post_id = target_post_id
    ),
    -- Find similar posts by tag overlap
    similar_posts AS (
        SELECT
            p.id,
            COUNT(DISTINCT pt.tag_id) as matching_tags,
            COUNT(DISTINCT pt2.tag_id) as total_tags,
            (COUNT(DISTINCT pt.tag_id)::float /
             NULLIF(COUNT(DISTINCT pt2.tag_id), 0)::float) as similarity
        FROM "global"."posts" p
        JOIN "global"."post_tags" pt2 ON p.id = pt2.post_id
        LEFT JOIN "global"."post_tags" pt ON pt2.tag_id = pt.tag_id AND pt.tag_id IN (SELECT tag_id FROM target_post_tags)
        WHERE p.id != target_post_id
          AND p.is_deleted = false
          AND p.is_draft = false
        GROUP BY p.id
        HAVING COUNT(DISTINCT pt.tag_id) > 0
        ORDER BY similarity DESC, p.views DESC
        LIMIT max_similar_posts
    )
    RETURN QUERY
    SELECT sp.id as post_id, sp.similarity as similarity_score
    FROM similar_posts sp;
END;
$$ LANGUAGE plpgsql;

-- Comment: To run these functions manually:
-- SELECT global.populate_recommendations();
-- 
-- To generate similar posts for a specific post:
-- SELECT * FROM global.generate_similar_posts(2);
--
-- These functions should be run periodically (e.g., daily) to keep recommendations fresh.
-- You can schedule them using pg_cron if available:
-- SELECT cron.schedule('0 1 * * *', 'SELECT global.populate_recommendations()'); 