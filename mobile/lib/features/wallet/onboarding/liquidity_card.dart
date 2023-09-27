import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/liquidity_option.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/wallet/onboarding/fund_wallet_modal.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:provider/provider.dart';

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
                      .catchError(
                          (error) => FLog.error(text: "Failed to create invoice! Error: $error"));
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
                    type: ValueType.amount,
                    value: widget.tradeUpTo,
                    label: "Trade up to",
                    valueTextStyle: fontStyle,
                    labelTextStyle: fontStyle,
                  ),
                ),
                const SizedBox(height: 5),
                Container(
                  padding: const EdgeInsets.fromLTRB(16, 0, 16, 0),
                  child: ValueDataRow(
                      type: ValueType.amount,
                      value: fee,
                      label: "Fee",
                      valueTextStyle: fontStyle,
                      labelTextStyle: fontStyle),
                ),
                const SizedBox(height: 5),
                Container(
                  padding: const EdgeInsets.fromLTRB(16, 0, 16, 0),
                  child: ValueDataRow(
                      type: ValueType.amount,
                      value: minDeposit,
                      label: "Min deposit",
                      valueTextStyle: fontStyle,
                      labelTextStyle: fontStyle),
                ),
                const SizedBox(height: 5),
                Container(
                  padding: const EdgeInsets.fromLTRB(16, 0, 16, 10),
                  child: ValueDataRow(
                    type: ValueType.amount,
                    value: widget.maxDeposit,
                    label: "Max deposit",
                    valueTextStyle: fontStyle,
                    labelTextStyle: fontStyle,
                  ),
                ),
              ]),
            )));
  }
}
