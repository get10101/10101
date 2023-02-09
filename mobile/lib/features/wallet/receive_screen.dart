import 'dart:developer';

import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/features/wallet/balance_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:provider/provider.dart';
import 'package:get_10101/ffi.dart';

class ReceiveScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "receive";

  const ReceiveScreen({super.key});

  @override
  State<ReceiveScreen> createState() => _ReceiveScreenState();
}

class _ReceiveScreenState extends State<ReceiveScreen> {
  String invoice = "";

  @override
  Widget build(BuildContext context) {
    Balance balance = context.watch<BalanceChangeNotifier>().balance;

    log("Refresh receive screen: ${balance.onChain}");

    return Scaffold(
      appBar: AppBar(title: const Text("Receive")),
      body: SafeArea(
          child: Column(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          const SizedBox(height: 50),
          SelectableText("Address: ${api.getNewAddress()}"),
          Text("Balance: ${balance.offChain} / ${balance.onChain}"),
          ElevatedButton(
              onPressed: () async {
                try {
                  setState(() async {
                    invoice = await api.createInvoice();
                  });

                  FLog.info(text: "Successfully created invoice.");
                } catch (error) {
                  FLog.error(text: "Error: $error", exception: error);
                }
              },
              child: const Text("Create Invoice")),
          SelectableText("Invoice: $invoice"),
          ElevatedButton(
            onPressed: () async {
              try {
                await api.openChannel();
                FLog.info(text: "Open Channel successfully started.");
              } catch (error) {
                FLog.error(text: "Error: $error", exception: error);
              }
            },
            child: const Text("Open Channel!"))
        ],
      )),
    );
  }
}
