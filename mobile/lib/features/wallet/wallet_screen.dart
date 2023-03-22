import 'package:flutter/material.dart';
import 'package:flutter_speed_dial/flutter_speed_dial.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/features/wallet/balance_row.dart';
import 'package:get_10101/features/wallet/create_invoice_screen.dart';
import 'package:get_10101/features/wallet/domain/wallet_history.dart';
import 'package:get_10101/features/wallet/wallet_history_item.dart';
import 'package:get_10101/features/wallet/wallet_theme.dart';
import 'package:get_10101/features/wallet/send_screen.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/util/send_receive_icons.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

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

    SizedBox listBottomScrollSpace = const SizedBox(
      height: 100,
    );

    return Scaffold(
      body: RefreshIndicator(
        onRefresh: () async {
          await walletChangeNotifier.refreshWalletInfo();
        },
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 20),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
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
                                    textStyle: const TextStyle(
                                        fontSize: 20.0, fontWeight: FontWeight.bold))),
                          )
                        ],
                      );
                    },
                    body: Padding(
                      padding: const EdgeInsets.only(left: 8.0, right: 8.0, bottom: 16.0),
                      child: Column(
                        children: const [
                          Padding(
                            padding: EdgeInsets.symmetric(horizontal: 8.0),
                            child: BalanceRow(walletType: WalletHistoryItemDataType.lightning),
                          ),
                          Padding(
                            padding: EdgeInsets.symmetric(horizontal: 8.0),
                            child: BalanceRow(walletType: WalletHistoryItemDataType.onChain),
                          )
                        ],
                      ),
                    ),
                    isExpanded: _isBalanceBreakdownOpen,
                  )
                ],
                expansionCallback: (i, isOpen) => setState(() => _isBalanceBreakdownOpen = !isOpen),
              ),
              Divider(color: theme.dividerColor),
              if (walletChangeNotifier.lightning().sats == 0)
                ElevatedButton(
                  onPressed: () {
                    context.go(CreateInvoiceScreen.route);
                  },
                  child: const Text("Fund Wallet"),
                ),
              const SizedBox(
                height: 10,
              ),
              Expanded(
                child: ListView.builder(
                  shrinkWrap: true,
                  physics: const ClampingScrollPhysics(),
                  itemCount: walletChangeNotifier.walletInfo.history.length + 1,
                  itemBuilder: (BuildContext context, int index) {
                    // Spacer at the bottom of the list
                    if (index == walletChangeNotifier.walletInfo.history.length) {
                      return listBottomScrollSpace;
                    }

                    WalletHistoryItemData itemData = walletChangeNotifier.walletInfo.history[index];

                    return WalletHistoryItem(
                      data: itemData,
                    );
                  },
                ),
              ),
            ],
          ),
        ),
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
