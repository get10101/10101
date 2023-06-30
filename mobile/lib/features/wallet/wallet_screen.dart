import 'package:flutter/material.dart';
import 'package:flutter_speed_dial/flutter_speed_dial.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/submission_status_dialog.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/wallet/seed_screen.dart';
import 'package:get_10101/features/wallet/send_payment_change_notifier.dart';
import 'package:get_10101/util/preferences.dart';
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
  Future<bool>? isUserSeedBackupConfirmed;

  @override
  void initState() {
    super.initState();
    isUserSeedBackupConfirmed = Preferences.instance.isUserSeedBackupConfirmed();
  }

  @override
  Widget build(BuildContext context) {
    WalletChangeNotifier walletChangeNotifier = context.watch<WalletChangeNotifier>();
    SendPaymentChangeNotifier sendPaymentChangeNotifier =
        context.watch<SendPaymentChangeNotifier>();

    if (sendPaymentChangeNotifier.pendingPayment != null &&
        sendPaymentChangeNotifier.pendingPayment!.state == PendingPaymentState.pending) {
      WidgetsBinding.instance.addPostFrameCallback((_) async {
        return await showDialog(
            context: context,
            useRootNavigator: true,
            builder: (BuildContext context) {
              return Selector<SendPaymentChangeNotifier, PendingPaymentState>(
                selector: (_, provider) => provider.pendingPayment!.state,
                builder: (context, state, child) {
                  const String title = "Send Payment";
                  Widget body = Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      SizedBox(
                        width: 200,
                        child: Wrap(
                          runSpacing: 10,
                          children: [
                            ValueDataRow(
                                type: ValueType.amount,
                                value: sendPaymentChangeNotifier
                                    .pendingPayment?.decodedInvoice.amountSats,
                                label: "Amount"),
                            ValueDataRow(
                                type: ValueType.text,
                                value:
                                    sendPaymentChangeNotifier.pendingPayment?.decodedInvoice.payee,
                                label: "Recipient")
                          ],
                        ),
                      ),
                      Padding(
                        padding: const EdgeInsets.only(top: 20, left: 10, right: 10, bottom: 5),
                        child: Text(
                            "Your Payment will be shown up in the wallet history automatically once it has been processed!",
                            style: DefaultTextStyle.of(context).style.apply(fontSizeFactor: 1.0)),
                      )
                    ],
                  );

                  switch (state) {
                    case PendingPaymentState.pending:
                      return SubmissionStatusDialog(
                          title: title, type: SubmissionStatusDialogType.pending, content: body);
                    case PendingPaymentState.succeeded:
                      return SubmissionStatusDialog(
                          title: title, type: SubmissionStatusDialogType.success, content: body);
                    case PendingPaymentState.failed:
                      return SubmissionStatusDialog(
                          title: title, type: SubmissionStatusDialogType.failure, content: body);
                  }
                },
              );
            });
      });
    }

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
                          const SizedBox(width: 64),
                          // ExpansionPanelList IconContainer size: end margin 8 + padding 16*2 + size 24),
                          Expanded(
                            child: Center(
                                child: walletChangeNotifier.syncing
                                    ? const Text(
                                        'Wallet syncing',
                                        style: TextStyle(
                                          fontWeight: FontWeight.bold,
                                          fontStyle: FontStyle.italic,
                                        ),
                                      )
                                    : AmountText(
                                        amount: walletChangeNotifier.total(),
                                        textStyle: const TextStyle(
                                            fontSize: 20.0, fontWeight: FontWeight.bold))),
                          )
                        ],
                      );
                    },
                    body: const Padding(
                      padding: EdgeInsets.only(left: 8.0, right: 8.0, bottom: 16.0),
                      child: Column(
                        children: [
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
              FutureBuilder(
                  future: isUserSeedBackupConfirmed,
                  builder: (BuildContext context, AsyncSnapshot<bool> snapshot) {
                    // FIXME: We ignore the value of `isUserSeedBackupConfirmed` stored in
                    // `snapshot.data` to keep the `Backup Wallet` button visible at all times for
                    // now. We need to rework this.
                    if (snapshot.connectionState == ConnectionState.done) {
                      return Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          const SizedBox(height: 3),
                          ElevatedButton(
                            onPressed: () async {
                              final res = await context.push(SeedScreen.route);

                              setState(() {
                                isUserSeedBackupConfirmed = Future.value(res as bool);
                              });
                            },
                            child: const Text("Backup Wallet"),
                          ),
                        ],
                      );
                    }
                    // return an empty box if the wallet has already been backed up or the data has not been fetched yet.
                    return const SizedBox(height: 0);
                  }),
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
