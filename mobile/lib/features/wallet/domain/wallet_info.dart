import 'transaction.dart';
import 'wallet_balances.dart';

class WalletInfo {
  WalletBalances balances;
  List<Transaction> history;

  WalletInfo({required this.balances, required this.history});
}
