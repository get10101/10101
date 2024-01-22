import 'package:flutter/material.dart';
import 'package:get_10101/common/payment.dart';
import 'package:get_10101/wallet/onchain_payment_history_item.dart';
import 'package:get_10101/wallet/wallet_service.dart';
import 'package:provider/provider.dart';

class HistoryScreen extends StatefulWidget {
  const HistoryScreen({super.key});

  @override
  State<HistoryScreen> createState() => _HistoryScreenState();
}

class _HistoryScreenState extends State<HistoryScreen> {
  List<OnChainPayment> history = [];

  @override
  void initState() {
    super.initState();
    context
        .read<WalletService>()
        .getOnChainPaymentHistory()
        .then((value) => setState(() => history = value));
  }

  @override
  Widget build(BuildContext context) {
    return Container(
        padding: const EdgeInsets.only(top: 25),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: Column(
                children: history.map((item) => OnChainPaymentHistoryItem(data: item)).toList(),
              ),
            ),
          ],
        ));
  }
}
