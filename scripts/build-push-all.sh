#!/bin/bash
docker build -t mpwsh/operator:latest --target operator .
docker build -t mpwsh/server:latest --target server .
docker build -t mpwsh/console:latest --target console .
docker push mpwsh/operator:latest
docker push mpwsh/server:latest
docker push mpwsh/console:latest
