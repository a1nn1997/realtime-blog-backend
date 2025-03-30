-- Insert a test admin user
INSERT INTO global.users (id, username, email, password_hash, role, created_at, updated_at)
VALUES (
    '550e8400-e29b-41d4-a716-446655440000'::uuid, 
    'admin', 
    'admin@example.com', 
    '$2b$12$1234567890123456789012uvwxyzABCDEFGHIJKLMNOPqrstuvwxyz', -- Example hashed password
    'admin',
    NOW(),
    NOW()
) ON CONFLICT (email) DO NOTHING;

-- Insert a test regular user
INSERT INTO global.users (id, username, email, password_hash, role, created_at, updated_at)
VALUES (
    '550e8400-e29b-41d4-a716-446655440001'::uuid, 
    'user', 
    'user@example.com', 
    '$2b$12$1234567890123456789012uvwxyzABCDEFGHIJKLMNOPqrstuvwxyz', -- Example hashed password
    'user',
    NOW(),
    NOW()
) ON CONFLICT (email) DO NOTHING; 