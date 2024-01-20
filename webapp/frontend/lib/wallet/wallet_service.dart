import 'package:get_10101/common/model.dart';
import 'package:get_10101/common/balance.dart';

class WalletService {
  const WalletService();

  Future<Balance> getBalance() async {
    // todo: fetch balance from backend
    return Balance(Amount(123454), Amount(124145214));
  }

  Future<String> getNewAddress() async {
    // todo: fetch new address from backend
    return "bcrt1qumc7lskp8x7947kw4culw2weld6axgrgz3nqqf";
  }

  Future<void> sendPayment(String address, Amount amount, Amount fee) async {
    // todo: send payment
    throw UnimplementedError("todo");
  }
}
