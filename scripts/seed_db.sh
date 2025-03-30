#!/bin/bash

# Exit on error
set -e

# Load environment variables from .env
if [ -f .env ]; then
    export $(grep -v '^#' .env | xargs)
    echo "Loaded environment variables from .env"
else
    echo "No .env file found. Make sure DATABASE_URL is set in your environment."
fi

# Check if DATABASE_URL is set
if [ -z "$DATABASE_URL" ]; then
    echo "Error: DATABASE_URL is not set. Please create a .env file with DATABASE_URL."
    exit 1
fi

echo "Seeding database with test data..."

# Extract connection parameters from DATABASE_URL
# Example: postgres://username:password@localhost:5432/dbname
DB_USER=$(echo $DATABASE_URL | awk -F'//' '{print $2}' | awk -F':' '{print $1}')
DB_PASS=$(echo $DATABASE_URL | awk -F':' '{print $3}' | awk -F'@' '{print $1}')
DB_HOST=$(echo $DATABASE_URL | awk -F'@' '{print $2}' | awk -F':' '{print $1}')
DB_PORT=$(echo $DATABASE_URL | awk -F':' '{print $4}' | awk -F'/' '{print $1}')
DB_NAME=$(echo $DATABASE_URL | awk -F'/' '{print $4}' | awk -F'?' '{print $1}')

# Run the schema SQL file using psql
PGPASSWORD=$DB_PASS psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -f scripts/seed_data.sql

echo "Database seeded successfully!" 