#!/usr/bin/env bash

URL=http://127.0.0.1:8080/actors

curl -s -X POST $URL/counter:1/call -d '{"action":"inc"}'; echo
curl -s -X POST $URL/counter:1/call -d '{"action":"inc"}'; echo
curl -s -X POST $URL/counter:1/call -d '{"action":"inc"}'; echo
curl -s -X POST $URL/counter:1/call -d '{"action":"get"}'; echo

echo "waiting 12s, counter:1 should go to sleep..."
sleep 12

curl -s -X POST $URL/counter:1/call -d '{"action":"get"}'; echo
curl -s -X POST $URL/counter:1/call -d '{"action":"inc"}'; echo
