
cd ../banco/
cargo build

./target/debug/banco &
export BANK=$!

echo "Bank  pid: $BANK"

#####

cd ../aerolineas/
cargo build

./target/debug/aerolineas &
export AIRLINE=$!

echo "Airline  pid: $AIRLINE"

#####

cd ../hotel/
cargo build

./target/debug/hotel &
export HOTEL=$!

echo "Hotel  pid: $HOTEL"


###
cd ../alglobo/
echo "$BANK,$AIRLINE,$HOTEL" > "processes.pid"