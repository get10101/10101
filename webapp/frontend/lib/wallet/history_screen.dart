import 'package:flutter/material.dart';
import 'package:get_10101/wallet/onchain_payment_history_item.dart';
import 'package:get_10101/wallet/wallet_change_notifier.dart';
import 'package:provider/provider.dart';

class HistoryScreen extends StatelessWidget {
  const HistoryScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final walletChangeNotifier = context.watch<WalletChangeNotifier>();

    final history = walletChangeNotifier.getHistory();

    return Container(
        padding: const EdgeInsets.only(top: 25),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(
              child: Column(
                children: history == null
                    ? [
                        const SizedBox(
                          width: 20,
                          height: 20,
                          child: CircularProgressIndicator(),
                        )
                      ]
                    : history.map((item) => OnChainPaymentHistoryItem(data: item)).toList(),
              ),
            ),
          ],
        ));
  }
}
