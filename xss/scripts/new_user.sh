#!/bin/bash

curl -X POST -H "Content-Type: application/json" -d '{"username":"t1","password":"test"}' http://127.0.0.1:10102/admin/register
curl -X POST -H "Content-Type: application/json" -d '{"username":"t2","password":"test"}' http://127.0.0.1:10102/admin/register
curl -X POST -H "Content-Type: application/json" -d '{"username":"test","password":"test"}' http://127.0.0.1:10102/admin/register
