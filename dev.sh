#!/bin/bash

docker build --iidfile ./iidfile .

SECCOMP_ARGS="--security-opt seccomp=$(pwd)/profiles/docker-localbind-seccomp-profile.json"
APPARMOR_ARGS="--security-opt apparmor=docker_localbind"

if [ ! -d /sys/kernel/security/apparmor ]; then
	APPARMOR_ARGS=""
fi

docker run --rm -it \
    $SECCOMP_ARGS \
    $APPARMOR_ARGS \
    --security-opt no-new-privileges:true \
    --cap-drop=ALL \
    -v "$(pwd):/home/user/docker-localbind" \
    -w /home/user/docker-localbind \
    "$(cat ./iidfile)" \
    /bin/bash
