import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/settings_screen.dart';
import 'package:go_router/go_router.dart';

class TradeScreen extends StatefulWidget {
  static const route = "/trade";
  static const label = "Trade";

  const TradeScreen({Key? key}) : super(key: key);

  @override
  State<TradeScreen> createState() => _TradeScreenState();
}

class _TradeScreenState extends State<TradeScreen> {
  @override
  Widget build(BuildContext context) {
    return Scaffold(
        body: ListView(
      padding: const EdgeInsets.only(left: 25, right: 25),
      children: const [Center(child: Text("Trade Screen"))],
    ));
  }
}
