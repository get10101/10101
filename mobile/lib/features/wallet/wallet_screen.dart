import 'package:flutter/material.dart';
import 'package:flutter_speed_dial/flutter_speed_dial.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/features/wallet/balance_row.dart';
import 'package:get_10101/features/wallet/create_invoice_screen.dart';
import 'package:get_10101/features/wallet/wallet_theme.dart';
import 'package:get_10101/features/wallet/send_screen.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/util/send_receive_icons.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'domain/wallet_type.dart';

class WalletScreen extends StatefulWidget {
  static const route = "/wallet";
  static const label = "Wallet";

  const WalletScreen({Key? key}) : super(key: key);

  @override
  State<WalletScreen> createState() => _WalletScreenState();
}

class _WalletScreenState extends State<WalletScreen> {
  bool _isBalanceBreakdownOpen = false;

  @override
  Widget build(BuildContext context) {
    WalletChangeNotifier walletChangeNotifier = context.watch<WalletChangeNotifier>();
    WalletTheme theme = Theme.of(context).extension<WalletTheme>()!;

    return Scaffold(
      body: ListView(
        padding: const EdgeInsets.only(left: 25, right: 25),
        children: [
          ExpansionPanelList(
            children: [
              ExpansionPanel(
                headerBuilder: (BuildContext context, bool isExpanded) {
                  return Row(
                    children: [
                      // https://stackoverflow.com/a/70192038 - do not know if this is principled
                      const SizedBox(
                          width:
                              64), // ExpansionPanelList IconContainer size: end margin 8 + padding 16*2 + size 24),
                      Expanded(
                        child: Center(
                            child: AmountText(
                                amount: walletChangeNotifier.total(),
                                textStyle: const TextStyle(fontSize: 20.0))),
                      )
                    ],
                  );
                },
                body: Padding(
                  padding: const EdgeInsets.only(left: 8.0, right: 8.0, bottom: 8.0),
                  child: Column(
                    children: WalletType.values
                        .map((type) => Padding(
                              padding: const EdgeInsets.all(8.0),
                              child: BalanceRow(walletType: type),
                            ))
                        .toList(growable: false),
                  ),
                ),
                isExpanded: _isBalanceBreakdownOpen,
              )
            ],
            expansionCallback: (i, isOpen) => setState(() => _isBalanceBreakdownOpen = !isOpen),
          ),
          Divider(color: theme.dividerColor),
          ElevatedButton(
            onPressed: () {
              context.go(CreateInvoiceScreen.route);
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
        overlayColor: theme.dividerColor,
        overlayOpacity: 0.5,
        elevation: 8.0,
        shape: const CircleBorder(),
        children: [
          SpeedDialChild(
            child: const Icon(SendReceiveIcons.receive, size: 20.0),
            label: 'Receive',
            labelStyle: const TextStyle(fontSize: 18.0),
            onTap: () => context.go(CreateInvoiceScreen.route),
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
