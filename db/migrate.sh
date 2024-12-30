#!/bin/bash
export $(envsubst < .env | xargs)
refinery migrate -e DATABASE_URL -p migrations/
