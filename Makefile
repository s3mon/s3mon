.PHONY: all build

all: build
	docker run -it --rm -e SLACK_WEBHOOK_URL=https://hooks.slack.com/services --name s3mon s3mon

build:
	docker build -t s3mon .
