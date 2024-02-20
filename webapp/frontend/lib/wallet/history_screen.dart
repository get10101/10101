import 'package:flutter/material.dart';
import 'package:get_10101/wallet/onchain_payment_history_item.dart';
import 'package:get_10101/wallet/wallet_change_notifier.dart';
import 'package:get_10101/wallet/wallet_service.dart';
import 'package:provider/provider.dart';

class HistoryScreen extends StatefulWidget {
  const HistoryScreen({super.key});

  @override
  State<HistoryScreen> createState() => _HistoryScreenState();
}

class _HistoryScreenState extends State<HistoryScreen> {
  bool refreshing = false;
  @override
  Widget build(BuildContext context) {
    final walletChangeNotifier = context.watch<WalletChangeNotifier>();

    final WalletService service = context.read<WalletService>();

    final history = walletChangeNotifier.getHistory();

    return Container(
        padding: const EdgeInsets.only(top: 25),
        child: Column(
          children: [
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Expanded(
                  child: ElevatedButton(
                      onPressed: refreshing
                          ? null
                          : () async {
                              setState(() {
                                refreshing = true;
                              });
                              await service.sync();
                              setState(() {
                                refreshing = false;
                              });
                            },
                      child:
                          refreshing ? const CircularProgressIndicator() : const Text("Refresh")),
                ),
              ],
            ),
            Row(
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
            ),
          ],
        ));
  }
}
