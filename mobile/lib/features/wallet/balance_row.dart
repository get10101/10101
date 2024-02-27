import 'package:flutter/material.dart';
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

    final offchainBalance = walletChangeNotifier.offChain()?.formatted() ?? "n/a";
    final onchainBalance = walletChangeNotifier.onChain()?.formatted() ?? "n/a";

    final amountText = switch (widget.walletType) {
      WalletType.lightning => Row(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Text(offchainBalance,
                  style: const TextStyle(
                      fontSize: 30, color: Colors.white, fontWeight: FontWeight.bold)),
              walletChangeNotifier.offChain() != null
                  ? const Text(" sats",
                      style: TextStyle(
                          fontSize: 14, color: Colors.white, fontWeight: FontWeight.normal))
                  : Container()
            ]),
      WalletType.onChain => Row(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Text(onchainBalance,
                  style: const TextStyle(
                      fontSize: 30, color: Colors.white, fontWeight: FontWeight.bold)),
              walletChangeNotifier.onChain() != null
                  ? const Text(" sats",
                      style: TextStyle(
                          fontSize: 14, color: Colors.white, fontWeight: FontWeight.normal))
                  : Container()
            ]),
    };

    return Center(child: amountText);
  }
}
