import sys
from random import randint
LIMIT = 100
PATH = 'src/alglobo/transactions'
def transactionsGenerator():
	if len(sys.argv) != 2:
		print(f"The amount of transactions is required")
		return
	amountOfTransactions = int(sys.argv[1])
	with open(PATH, 'w') as transactionsFile:
		for id in range(amountOfTransactions):
			transactionId = 'id_' + str(id)
			paymentAirline = randint(-LIMIT, LIMIT)
			paymentBank = randint(-LIMIT, LIMIT)
			paymentHotel = randint(-LIMIT, LIMIT)
			transactionsFile.write(f"{transactionId},{paymentAirline},{paymentBank},{paymentHotel}\n")
	print('Transactions created!')

transactionsGenerator()
