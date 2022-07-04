#!/bin/bash

PIDS=`cat processes.pid`
echo "Pids: $PIDS"

BANK=`sed 's/\(.*\),.*,.*/\1/g' processes.pid`
AIRLINE=`sed 's/.*,\(.*\),.*/\1/g' processes.pid`
HOTEL=`sed 's/.*,.*,\(.*\)/\1/g' processes.pid`

echo "Killing Bank.."
kill $BANK

echo "Killing Airline.."
kill $AIRLINE

echo "Killing Hotel.."
kill $HOTEL
