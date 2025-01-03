#!/bin/bash
export $(envsubst < .env | xargs)
refinery migrate -e DATABASE_URL -p migrations/
# Export current schema to schema.sql
pg_dump -s "$DATABASE_URL" > schema.sql
