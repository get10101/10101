import 'package:get_10101/common/amount.dart';
import 'package:get_10101/common/balance.dart';

class WalletService {
  const WalletService();

  Future<Balance> getBalance() async {
    // todo: fetch balance from backend
    return Balance(Amount(123454), Amount(124145214));
  }
}
