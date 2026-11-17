#!/usr/bin/env bash

URL=http://127.0.0.1:8080/actors

echo "=== counter"
curl -s -X POST $URL/counter:1/call -d '{"action":"inc"}'; echo
curl -s -X POST $URL/counter:1/call -d '{"action":"inc"}'; echo
curl -s -X POST $URL/counter:1/call -d '{"action":"inc"}'; echo
curl -s -X POST $URL/counter:1/call -d '{"action":"get"}'; echo

KB=$URL/kb:test-$(date +%s)/call

echo
echo "=== kb: put 4 chunks (vectors have 3 numbers)"
curl -s -X POST $KB -d '{"action":"put","args":{"text":"about cats","tag":"ch1","vector":[1,0,0]}}'; echo
curl -s -X POST $KB -d '{"action":"put","args":{"text":"cats and dogs","tag":"ch2","vector":[0.7,0.7,0]}}'; echo
curl -s -X POST $KB -d '{"action":"put","args":{"text":"about dogs","tag":"ch2","vector":[0,1,0]}}'; echo
curl -s -X POST $KB -d '{"action":"put","args":{"text":"opposite of cats","vector":[-1,0,0]}}'; echo

echo
echo "=== kb: bad puts, each should be an error"
curl -s -X POST $KB -d '{"action":"put","args":{"text":"wrong size","vector":[1,0]}}'; echo
curl -s -X POST $KB -d '{"action":"put","args":{"text":"zeros","vector":[0,0,0]}}'; echo
curl -s -X POST $KB -d '{"action":"put","args":{"text":"not numbers","vector":[1,"x",0]}}'; echo
curl -s -X POST $KB -d '{"action":"put","args":{"vector":[1,0,0]}}'; echo

echo
echo "=== kb: search near cats, expect cats first, opposite last"
curl -s -X POST $KB -d '{"action":"search","args":{"vector":[1,0.1,0],"limit":5}}'; echo

echo
echo "=== kb: same search, only tag ch2"
curl -s -X POST $KB -d '{"action":"search","args":{"vector":[1,0.1,0],"tag":"ch2"}}'; echo

echo
echo "=== kb: best 1 only, with its vector"
curl -s -X POST $KB -d '{"action":"search","args":{"vector":[1,0.1,0],"limit":1,"with_vector":true}}'; echo

echo
echo "=== kb: unknown tag, expect no results"
curl -s -X POST $KB -d '{"action":"search","args":{"vector":[1,0.1,0],"tag":"nope"}}'; echo

echo
echo "=== kb: bad searches, each should be an error"
curl -s -X POST $KB -d '{"action":"search","args":{"vector":[1,0]}}'; echo
curl -s -X POST $KB -d '{"action":"search","args":{"vector":[0,0,0]}}'; echo

echo
echo "=== kb: get (debug view)"
curl -s -X POST $KB -d '{"action":"get"}'; echo

TEXT_KB=$URL/kb:text-$(date +%s)/call

echo
echo "=== kb with a local embedding model (server needs EMBED_URL and EMBED_MODEL set)"
curl -s -X POST $TEXT_KB -d '{"action":"put_text","args":{"text":"Cats are small furry pets that purr and meow","tag":"animals"}}'; echo
curl -s -X POST $TEXT_KB -d '{"action":"put_text","args":{"text":"Rust has no garbage collector; ownership frees memory","tag":"rust"}}'; echo
curl -s -X POST $TEXT_KB -d '{"action":"put_text","args":{"text":"Dogs are loyal pets that bark and love walks","tag":"animals"}}'; echo

echo
echo "=== search_text: a question about memory, expect the rust chunk first"
curl -s -X POST $TEXT_KB -d '{"action":"search_text","args":{"query":"how is memory managed without a GC?","limit":2}}'; echo

echo
echo "=== search_text: only tag animals, expect cats first"
curl -s -X POST $TEXT_KB -d '{"action":"search_text","args":{"query":"which pet meows?","tag":"animals"}}'; echo

echo
echo "waiting 12s, counter:1 and the kb should go to sleep..."
sleep 12

echo
echo "=== after sleep: counter keeps its count, kb wakes from disk with all 4 chunks"
curl -s -X POST $URL/counter:1/call -d '{"action":"get"}'; echo
curl -s -X POST $KB -d '{"action":"get"}'; echo
curl -s -X POST $KB -d '{"action":"search","args":{"vector":[0,1,0],"limit":2}}'; echo
