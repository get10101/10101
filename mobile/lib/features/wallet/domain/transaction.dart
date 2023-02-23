import 'package:get_10101/common/domain/model.dart';

import 'payment_flow.dart';
import 'wallet_type.dart';

class Transaction {
  final String address;
  final PaymentFlow flow;
  final Amount amount;
  final WalletType walletType;

  const Transaction(
      {required this.address, required this.flow, required this.amount, required this.walletType});
}
