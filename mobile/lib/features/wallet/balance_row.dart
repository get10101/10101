import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/fiat_text.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:provider/provider.dart';

class BalanceRow extends StatefulWidget {
  final WalletType walletType;
  final double iconSize;

  const BalanceRow({required this.walletType, this.iconSize = 30, super.key});

  @override
  State<BalanceRow> createState() => _BalanceRowState();
}

class _BalanceRowState extends State<BalanceRow> with SingleTickerProviderStateMixin {
  @override
  Widget build(BuildContext context) {
    WalletChangeNotifier walletChangeNotifier = context.watch<WalletChangeNotifier>();
    const normal = TextStyle(fontSize: 16.0);
    const bold = TextStyle(fontWeight: FontWeight.bold, fontSize: 16.0);

    PositionChangeNotifier positionChangeNotifier = context.watch<PositionChangeNotifier>();

    final (name, icon, amountText) = switch (widget.walletType) {
      WalletType.lightning => (
          "Lightning",
          Icons.bolt,
          AmountText(amount: walletChangeNotifier.lightning(), textStyle: bold),
        ),
      WalletType.onChain => (
          "On-chain",
          Icons.currency_bitcoin,
          AmountText(amount: walletChangeNotifier.onChain(), textStyle: bold),
        ),
      WalletType.stable => (
          "USDP",
          Icons.attach_money,
          FiatText(
            amount: positionChangeNotifier.getStableUSDAmountInFiat(),
            textStyle: bold,
          )
        ),
    };

    double balanceRowHeight = 50;

    return SizedBox(
      height: balanceRowHeight,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 4.0),
        child: Row(children: [
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 4.0),
            child: Icon(icon, color: tenTenOnePurple),
          ),
          Expanded(child: Text(name, style: normal)),
          amountText,
        ]),
      ),
    );
  }
}
