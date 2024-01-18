import 'package:flutter/material.dart';

class WalletScreen extends StatefulWidget {
  static const route = "/wallet";

  const WalletScreen({super.key});

  @override
  State<WalletScreen> createState() => _WalletScreenState();
}

class _WalletScreenState extends State<WalletScreen> {
  @override
  Widget build(BuildContext context) {
    return const Column(
      mainAxisAlignment: MainAxisAlignment.start,
      crossAxisAlignment: CrossAxisAlignment.center,
      children: <Widget>[
        Text(
          'Wallet Screen',
        ),
      ],
    );
  }
}
