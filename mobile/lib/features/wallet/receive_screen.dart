import 'package:flutter/material.dart';
import 'package:get_10101/features/wallet/balance_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:provider/provider.dart';
import 'package:get_10101/ffi.dart';

class ReceiveScreen extends StatelessWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "receive";

  const ReceiveScreen({super.key});

  @override
  Widget build(BuildContext context) {
    Balance balance = context.watch<BalanceChangeNotifier>().balance;

    return Scaffold(
      appBar: AppBar(title: const Text("Receive")),
      body: SafeArea(
          child: Column(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          const SizedBox(height: 50),
          SelectableText("Address: ${api.getNewAddress()}"),
          Text("Balance: ${balance.offChain} / ${balance.onChain}"),
        ],
      )),
    );
  }
}
