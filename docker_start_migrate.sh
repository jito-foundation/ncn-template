#! /bin/bash

docker compose --env-file cli/.env up -d --build ncn-program-ncn-keeper-migrate-only --remove-orphans