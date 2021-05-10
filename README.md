# [Crusty Cards](https://crusty.cards/) Backend Monorepo
[![Prod CI/CD](https://github.com/crustycards/backend/actions/workflows/prod-ci.yml/badge.svg?branch=master)](https://github.com/crustycards/backend/actions/workflows/prod-ci.yml)

Contains code for API Service and Game Service.

API Service Responsibilities:
* Reading and writing cardpack and user data to/from the main database
* Keeping the search index in sync with the main database

Game Service Responsibilities:
* Keeping all active game data in memory
* Pushing live game updates to RabbitMQ

See the full inter-service architectural diagram [here](https://app.moqups.com/Syjv300SBW/view/page/a46483b7c?fit_width=1).