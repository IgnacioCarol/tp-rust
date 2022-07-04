#!/bin/bash

PIDS=`cat proccess.pid`
echo "Pids: $PIDS"

BANK=`sed 's/\(.*\),.*,.*/\1/g' proccess.pid`
AIRLINE=`sed 's/.*,\(.*\),.*/\1/g' proccess.pid`
HOTEL=`sed 's/.*,.*,\(.*\)/\1/g' proccess.pid`

echo "Killing Bank.."
kill $BANK

echo "Killing Airline.."
kill $AIRLINE

echo "Killing Hotel.."
kill $HOTEL
