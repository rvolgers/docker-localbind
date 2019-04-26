#!/bin/bash

docker build --iidfile ./iidfile .

docker run --rm -it -v "$(pwd)/test-bindfstab:/etc/bindfstab" -v "$(pwd):/home/user/docker-localbind" --security-opt seccomp="$(pwd)/docker-localbind-seccomp-profile.json" -w /home/user/docker-localbind $(cat ./iidfile) /bin/bash


