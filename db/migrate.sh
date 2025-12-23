#!/bin/bash
export $(envsubst < .env | xargs)
refinery migrate -e DATABASE_URL -p migrations/
# Export current schema to schema.sql for AI to read
pg_dump -s "$DATABASE_URL" | tr -d '\r' > schema.sql
# Export functions and procedures to functions.sql for AI to read
psql "$DATABASE_URL" -c "SELECT pg_get_functiondef(oid) || ';' FROM pg_proc WHERE pronamespace = 'public'::regnamespace ORDER BY proname;" -t -o functions.sql
