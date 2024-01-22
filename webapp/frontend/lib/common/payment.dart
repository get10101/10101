import 'package:get_10101/common/model.dart';

class Payment {
  final String address;
  final int amount;
  final int fee;

  const Payment({required this.address, required this.amount, required this.fee});
}

enum PaymentFlow { outbound, inbound }

class OnChainPayment {
  final PaymentFlow flow;
  final Amount amount;
  final DateTime timestamp;
  final String txid;
  final int confirmations;
  final Amount? fee;

  const OnChainPayment(
      {required this.flow,
      required this.amount,
      required this.timestamp,
      required this.txid,
      required this.confirmations,
      this.fee});

  factory OnChainPayment.fromJson(Map<String, dynamic> json) {
    return switch (json) {
      {
        "flow": String flow,
        "amount": int amount,
        "timestamp": int timestamp,
        "txid": String txid,
        "confirmations": int confirmations,
        "fee": int fee
      } =>
        OnChainPayment(
            flow: flow == 'inbound' ? PaymentFlow.inbound : PaymentFlow.outbound,
            amount: Amount(amount),
            timestamp: DateTime.fromMillisecondsSinceEpoch(timestamp * 1000),
            txid: txid,
            confirmations: confirmations,
            fee: Amount(fee)),
      _ => throw const FormatException('Failed to load history.'),
    };
  }
}
