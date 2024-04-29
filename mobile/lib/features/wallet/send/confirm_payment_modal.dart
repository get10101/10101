import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/features/wallet/application/util.dart';
import 'package:get_10101/features/wallet/domain/destination.dart';
import 'package:get_10101/features/wallet/domain/fee.dart';
import 'package:get_10101/features/wallet/domain/fee_estimate.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'package:slide_to_confirm/slide_to_confirm.dart';

void showConfirmPaymentModal(BuildContext context, Destination destination, Amount? sats,
    FeeConfig feeConfig, FeeEstimation feeEstimation) {
  showModalBottomSheet<void>(
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(
          top: Radius.circular(20),
        ),
      ),
      clipBehavior: Clip.antiAlias,
      isScrollControlled: true,
      useRootNavigator: false,
      context: context,
      builder: (BuildContext context) {
        return ConfirmPayment(
          destination: destination,
          amt: sats,
          feeConfig: feeConfig,
          feeEstimation: feeEstimation,
        );
      });
}

class ConfirmPayment extends StatelessWidget {
  final Destination destination;
  final Amount? amt;
  final FeeConfig feeConfig;
  final FeeEstimation feeEstimation;

  const ConfirmPayment({
    super.key,
    required this.destination,
    required this.amt,
    required this.feeConfig,
    required this.feeEstimation,
  });

  @override
  Widget build(BuildContext context) {
    final walletService = context.read<WalletChangeNotifier>().service;

    final tradeValuesChangeNotifier = context.watch<TradeValuesChangeNotifier>();

    final tradeValues = tradeValuesChangeNotifier.fromDirection(Direction.long);
    tradeValues.updateLeverage(Leverage(1));

    return SafeArea(
      child: Container(
        color: Colors.white,
        padding: const EdgeInsets.only(left: 20.0, top: 35.0, right: 20.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.center,
          mainAxisSize: MainAxisSize.min,
          children: [
            Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Text("Summary", style: TextStyle(fontSize: 20)),
                const SizedBox(height: 32),
                Container(
                  padding: const EdgeInsets.all(20),
                  decoration: BoxDecoration(
                      color: tenTenOnePurple.shade200.withOpacity(0.1),
                      borderRadius: BorderRadius.circular(8)),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      const Text("Amount", style: TextStyle(color: Colors.grey, fontSize: 16)),
                      const SizedBox(height: 5),
                      Text(amt == null ? "Max" : amt.toString(),
                          style: const TextStyle(fontSize: 16)),
                      const Divider(height: 40, indent: 0, endIndent: 0),
                      const Text("Destination", style: TextStyle(color: Colors.grey, fontSize: 16)),
                      const SizedBox(height: 5),
                      Text(truncateWithEllipsis(26, destination.raw),
                          style: const TextStyle(fontSize: 16)),
                      const Divider(height: 40, indent: 0, endIndent: 0),
                      Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
                        Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          mainAxisAlignment: MainAxisAlignment.start,
                          children: [
                            const Text("Fee", style: TextStyle(fontSize: 16)),
                            if (feeConfig is PriorityFee)
                              Text("(${(feeConfig as PriorityFee).priority})",
                                  style: const TextStyle(fontSize: 16))
                            else if (feeConfig is CustomFeeRate)
                              const Text("(Custom)", style: TextStyle(fontSize: 16))
                          ],
                        ),
                        Column(crossAxisAlignment: CrossAxisAlignment.end, children: [
                          AmountText(
                            amount: feeEstimation.total,
                            textStyle: const TextStyle(fontSize: 16),
                          ),
                          // TODO: Estimate time for `CustomFee`.
                          if (feeConfig is PriorityFee)
                            Text((feeConfig as PriorityFee).priority.toTimeEstimate(),
                                style: const TextStyle(fontSize: 16, color: Colors.grey)),
                        ]),
                      ]),
                      const SizedBox(height: 10),
                    ],
                  ),
                )
              ],
            ),
            const SizedBox(height: 32),
            ConfirmationSlider(
                text: "Swipe to confirm",
                textStyle: const TextStyle(color: Colors.black87),
                height: 40,
                foregroundColor: tenTenOnePurple,
                sliderButtonContent: const Icon(
                  Icons.chevron_right,
                  color: Colors.white,
                  size: 20,
                ),
                onConfirmation: () async {
                  GoRouter.of(context).pop();
                  final goRouter = GoRouter.of(context);
                  final messenger = ScaffoldMessenger.of(context);
                  try {
                    var txid = await walletService.sendOnChainPayment(destination, amt,
                        feeConfig: feeConfig);
                    showSnackBar(messenger, "Transaction broadcasted $txid");
                    goRouter.pop();
                  } catch (error) {
                    logger.e("Failed to send payment: $error");
                    showSnackBar(messenger, error.toString());
                  }
                }),
            const SizedBox(height: 24),
          ],
        ),
      ),
    );
  }
}
