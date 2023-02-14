import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart';
import 'package:get_10101/features/wallet/balance_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:provider/provider.dart';

class ReceiveScreen extends StatelessWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "receive";

  const ReceiveScreen({super.key});

  @override
  Widget build(BuildContext context) {
    Balance balance = context.read<BalanceChangeNotifier>().balance;

    return Scaffold(
      appBar: AppBar(title: const Text("Receive")),
      body: SafeArea(
          child: Column(
        children: [
          Text("Balance: ${balance.offChain}"),
          ElevatedButton(onPressed: () async {}, child: const Text("Send me some money!"))
        ],
      )),
    );
  }
}
