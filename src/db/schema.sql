-- Database schema will be defined here

-- Create global schema if it doesn't exist
CREATE SCHEMA IF NOT EXISTS global;

-- Create users table
CREATE TABLE IF NOT EXISTS global.users (
    id UUID PRIMARY KEY,
    username VARCHAR(100) NOT NULL,
    email VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    role VARCHAR(20) NOT NULL DEFAULT 'user',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create posts table
CREATE TABLE IF NOT EXISTS global.posts (
    id BIGSERIAL PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL UNIQUE,
    content TEXT NOT NULL,
    content_html TEXT NOT NULL,
    user_id UUID NOT NULL REFERENCES global.users(id),
    views INTEGER NOT NULL DEFAULT 0,
    likes INTEGER NOT NULL DEFAULT 0,
    is_draft BOOLEAN NOT NULL DEFAULT FALSE,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    cover_image_url VARCHAR(1024),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create tags table
CREATE TABLE IF NOT EXISTS global.tags (
    id BIGSERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL UNIQUE
);

-- Create post_tags junction table
CREATE TABLE IF NOT EXISTS global.post_tags (
    post_id BIGINT NOT NULL REFERENCES global.posts(id) ON DELETE CASCADE,
    tag_id BIGINT NOT NULL REFERENCES global.tags(id) ON DELETE CASCADE,
    PRIMARY KEY (post_id, tag_id)
);

-- Create comments table
CREATE TABLE IF NOT EXISTS global.comments (
    id BIGSERIAL PRIMARY KEY,
    post_id BIGINT NOT NULL REFERENCES global.posts(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES global.users(id),
    parent_comment_id BIGINT REFERENCES global.comments(id),
    content TEXT NOT NULL,
    content_html TEXT NOT NULL,
    is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    deleted_by UUID REFERENCES global.users(id),
    deleted_at TIMESTAMPTZ,
    markdown_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Comments can only be nested to a certain depth (tracked for performance)
    nesting_level INTEGER NOT NULL DEFAULT 0 
);

-- Create indexes
CREATE INDEX IF NOT EXISTS idx_posts_user_id ON global.posts(user_id);
CREATE INDEX IF NOT EXISTS idx_posts_slug ON global.posts(slug);
CREATE INDEX IF NOT EXISTS idx_posts_created_at ON global.posts(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_post_tags_post_id ON global.post_tags(post_id);
CREATE INDEX IF NOT EXISTS idx_post_tags_tag_id ON global.post_tags(tag_id);
CREATE INDEX IF NOT EXISTS idx_tags_name ON global.tags(name);

-- Comment indexes
CREATE INDEX IF NOT EXISTS idx_comments_post_id ON global.comments(post_id);
CREATE INDEX IF NOT EXISTS idx_comments_user_id ON global.comments(user_id);
CREATE INDEX IF NOT EXISTS idx_comments_parent_id ON global.comments(parent_comment_id);
CREATE INDEX IF NOT EXISTS idx_comments_created_at ON global.comments(created_at DESC);
