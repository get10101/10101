import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:intl/intl.dart';
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

    PositionChangeNotifier positionChangeNotifier = context.watch<PositionChangeNotifier>();
    final formatter = NumberFormat("#,###,##0.00", "en");

    final amountText = switch (widget.walletType) {
      WalletType.lightning => Row(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Text(walletChangeNotifier.offChain().formatted(),
                  style: const TextStyle(
                      fontSize: 30, color: Colors.white, fontWeight: FontWeight.bold)),
              const Text(" sats",
                  style:
                      TextStyle(fontSize: 14, color: Colors.white, fontWeight: FontWeight.normal))
            ]),
      WalletType.onChain => Row(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Text(walletChangeNotifier.onChain().formatted(),
                  style: const TextStyle(
                      fontSize: 30, color: Colors.white, fontWeight: FontWeight.bold)),
              const Text(" sats",
                  style:
                      TextStyle(fontSize: 14, color: Colors.white, fontWeight: FontWeight.normal))
            ]),
      WalletType.stable => Row(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Text(formatter.format(positionChangeNotifier.getStableUSDAmountInFiat()),
                  style: const TextStyle(
                      fontSize: 30, color: Colors.white, fontWeight: FontWeight.bold)),
              const Text(" \$",
                  style:
                      TextStyle(fontSize: 14, color: Colors.white, fontWeight: FontWeight.normal))
            ]),
    };

    return Center(child: amountText);
  }
}
