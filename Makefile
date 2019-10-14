.PHONY: all build

all: build
	docker run -it --rm s3mon

build:
	docker build -t s3mon .
