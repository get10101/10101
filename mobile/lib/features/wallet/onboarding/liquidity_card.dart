import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/liquidity_option.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/wallet/onboarding/fund_wallet_modal.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

class LiquidityCard extends StatefulWidget {
  final int liquidityOptionId;
  final String title;
  final Amount tradeUpTo;
  final ProportionalFee fee;
  final Amount minDeposit;
  final Amount maxDeposit;
  final Amount? amount;
  final bool enabled;

  final Function onTap;

  const LiquidityCard(
      {super.key,
      required this.liquidityOptionId,
      required this.title,
      required this.tradeUpTo,
      required this.fee,
      required this.minDeposit,
      required this.maxDeposit,
      required this.amount,
      required this.enabled,
      required this.onTap});

  @override
  State<LiquidityCard> createState() => _LiquidityCardState();
}

class _LiquidityCardState extends State<LiquidityCard> {
  bool _onTap = false;

  @override
  Widget build(BuildContext context) {
    final walletService = context.read<WalletChangeNotifier>().service;
    final amount = widget.amount ?? Amount(0);
    final fee = widget.fee.getFee(amount);
    final minDeposit = widget.minDeposit.add(fee);
    final maxDeposit = widget.maxDeposit;

    const fontStyle = TextStyle(fontSize: 15);
    return GestureDetector(
        onTap: !widget.enabled
            ? null
            : () {
                widget.onTap(minDeposit, maxDeposit);
                if (amount.sats >= minDeposit.sats && amount.sats <= maxDeposit.sats) {
                  walletService
                      .createOnboardingInvoice(amount, widget.liquidityOptionId)
                      .then(
                          (invoice) => showFundWalletModal(context, widget.amount!, fee, invoice!))
                      .catchError((error) {
                    logger.e("Failed to create invoice!", error: error);
                    if (error is FfiException &&
                        error.message.contains("cannot provide required liquidity")) {
                      showNoLiquidityDialog();
                    }
                  });
                }
              },
        onTapDown: (details) {
          setState(() {
            _onTap = true;
          });
        },
        onTapUp: (details) {
          setState(() {
            _onTap = false;
          });
        },
        child: Card(
            elevation: _onTap ? 1.0 : 4.0,
            child: Container(
              margin: const EdgeInsets.fromLTRB(0, 3, 0, 10),
              child: Column(children: [
                ListTile(
                  title: Text(
                    widget.title,
                    style: TextStyle(
                      color: tenTenOnePurple.shade800,
                      fontSize: 23,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                ),
                Container(
                  padding: const EdgeInsets.fromLTRB(16, 0, 16, 0),
                  child: ValueDataRow(
                    value: AmountText(amount: widget.tradeUpTo),
                    label: "Trade up to",
                    valueTextStyle: fontStyle,
                    labelTextStyle: fontStyle,
                  ),
                ),
                const SizedBox(height: 5),
                Container(
                  padding: const EdgeInsets.fromLTRB(16, 0, 16, 0),
                  child: ValueDataRow(
                      value: AmountText(amount: fee),
                      label: "Fee",
                      valueTextStyle: fontStyle,
                      labelTextStyle: fontStyle),
                ),
                const SizedBox(height: 5),
                Container(
                  padding: const EdgeInsets.fromLTRB(16, 0, 16, 0),
                  child: ValueDataRow(
                      value: AmountText(amount: minDeposit),
                      label: "Min deposit",
                      valueTextStyle: fontStyle,
                      labelTextStyle: fontStyle),
                ),
                const SizedBox(height: 5),
                Container(
                  padding: const EdgeInsets.fromLTRB(16, 0, 16, 10),
                  child: ValueDataRow(
                    value: AmountText(amount: widget.maxDeposit),
                    label: "Max deposit",
                    valueTextStyle: fontStyle,
                    labelTextStyle: fontStyle,
                  ),
                ),
              ]),
            )));
  }
}

void showNoLiquidityDialog() {
  showDialog(
      context: rootNavigatorKey.currentContext!,
      builder: (context) => AlertDialog(
              title: const Text("No liquidity in the LSP."),
              content: RichText(
                text: TextSpan(
                  children: <TextSpan>[
                    const TextSpan(
                        text: "The LSP cannot temporarily open a channel to you.\n"
                            "Please try again later.\n\nIf you have further questions, please contact us on Telegram: ",
                        style: TextStyle(fontSize: 16, color: Colors.black)),
                    TextSpan(
                      text: 'https://t.me/get10101',
                      style: const TextStyle(fontSize: 16, color: Colors.blue),
                      recognizer: TapGestureRecognizer()
                        ..onTap = () async {
                          final httpsUri = Uri(scheme: 'https', host: 't.me', path: 'get10101');
                          if (await canLaunchUrl(httpsUri)) {
                            await launchUrl(httpsUri);
                          } else {
                            showSnackBar(ScaffoldMessenger.of(rootNavigatorKey.currentContext!),
                                "Failed to open link");
                          }
                        },
                    ),
                  ],
                ),
              ),
              actions: [
                TextButton(
                  onPressed: () {
                    GoRouter.of(context).pop();
                  },
                  child: const Text('OK'),
                ),
              ]));
}
