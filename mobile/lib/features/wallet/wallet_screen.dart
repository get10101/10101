import 'package:flutter/material.dart';
import 'package:flutter_speed_dial/flutter_speed_dial.dart';
import 'package:get_10101/features/wallet/receive_screen.dart';
import 'package:get_10101/features/wallet/send_screen.dart';
import 'package:get_10101/util/send_receive_icons.dart';
import 'package:go_router/go_router.dart';

class WalletScreen extends StatefulWidget {
  static const route = "/wallet";
  static const label = "Wallet";

  const WalletScreen({Key? key}) : super(key: key);

  @override
  State<WalletScreen> createState() => _WalletScreenState();
}

class _WalletScreenState extends State<WalletScreen> {
  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: ListView(
        padding: const EdgeInsets.only(left: 25, right: 25),
        children: [
          const Center(child: Text("Wallet Screen")),
          ElevatedButton(
            onPressed: () {
              context.go(ReceiveScreen.route);
            },
            child: const Text("Fund Wallet"),
          ),
        ],
      ),
      floatingActionButton: SpeedDial(
        icon: SendReceiveIcons.sendReceive,
        iconTheme: const IconThemeData(size: 20),
        activeIcon: Icons.close,
        buttonSize: const Size(56.0, 56.0),
        visible: true,
        closeManually: false,
        curve: Curves.bounceIn,
        overlayColor: Colors.black,
        overlayOpacity: 0.5,
        elevation: 8.0,
        shape: const CircleBorder(),
        children: [
          SpeedDialChild(
            child: const Icon(SendReceiveIcons.receive, size: 20.0),
            label: 'Receive',
            labelStyle: const TextStyle(fontSize: 18.0),
            onTap: () => context.go(ReceiveScreen.route),
          ),
          SpeedDialChild(
            child: const Icon(SendReceiveIcons.sendWithQr, size: 24.0),
            label: 'Send',
            labelStyle: const TextStyle(fontSize: 18.0),
            onTap: () => GoRouter.of(context).go(SendScreen.route),
          ),
        ],
      ),
    );
  }
}
