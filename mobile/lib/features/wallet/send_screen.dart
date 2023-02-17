import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/ffi.dart';

class SendScreen extends StatefulWidget {
  static const route = "${WalletScreen.route}/$subRouteName";
  static const subRouteName = "send";

  const SendScreen({super.key});

  @override
  State<SendScreen> createState() => _SendScreenState();
}

class _SendScreenState extends State<SendScreen> {
  String encodedInvoice = "";

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text("Send")),
      body: SafeArea(
          child: Column(
        children: [
          const SizedBox(height: 50),
          TextFormField(onChanged: (text) {
            setState(() {
              encodedInvoice = text;
            });
          }),
          ElevatedButton(
              onPressed: () async {
                try {
                  await api.sendPayment(invoice: encodedInvoice);
                  FLog.info(text: "Successfully payed invoice.");
                } catch (error) {
                  FLog.error(text: "Error: $error", exception: error);
                }
              },
              child: const Text("Send Payment")),
        ],
      )),
    );
  }
}
