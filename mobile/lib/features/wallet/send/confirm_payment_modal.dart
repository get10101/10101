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
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:go_router/go_router.dart';
import 'package:intl/intl.dart';
import 'package:provider/provider.dart';
import 'package:slide_to_confirm/slide_to_confirm.dart';

void showConfirmPaymentModal(
    BuildContext context, Destination destination, bool payWithUsdp, Amount sats, Usd usdp,
    {Fee? fee}) {
  logger.i(fee);
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
          payWithUsdp: payWithUsdp,
          destination: destination,
          sats: sats,
          usdp: usdp,
          fee: fee,
        );
      });
}

class ConfirmPayment extends StatelessWidget {
  final Destination destination;
  final bool payWithUsdp;
  final Amount sats;
  final Usd usdp;
  final Fee? fee;

  const ConfirmPayment(
      {super.key,
      required this.destination,
      required this.payWithUsdp,
      required this.sats,
      required this.usdp,
      this.fee});

  @override
  Widget build(BuildContext context) {
    final walletService = context.read<WalletChangeNotifier>().service;
    final formatter = NumberFormat("#,###,##0.00", "en");

    final tradeValuesChangeNotifier = context.watch<TradeValuesChangeNotifier>();

    final tradeValues = tradeValuesChangeNotifier.fromDirection(Direction.long);
    tradeValues.updateLeverage(Leverage(1));

    Amount amt = destination.amount;
    if (destination.amount.sats == 0) {
      if (payWithUsdp) {
        // if the destination does not specify an amount and we ar paying with the usdp balance we
        // calculate the amount from the quantity point of view.
        tradeValues.updateQuantity(usdp);
        amt = tradeValues.margin!;
      } else {
        // Otherwise it is a regular lightning payment and we just pay the given amount.
        amt = sats;
      }
    } else {
      // if the amount is set on the invoice we need to pay the amount no matter what. That might
      // lead to the usdp amount to jump by one dollar depending on the current bid price
      tradeValues.updateMargin(destination.amount);
    }

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
                      Visibility(
                          visible: payWithUsdp,
                          replacement: Text(amt.sats == 0 ? "Max" : amt.toString(),
                              style: const TextStyle(fontSize: 16)),
                          child: Row(
                            mainAxisAlignment: MainAxisAlignment.spaceBetween,
                            children: [
                              Text(
                                  "~ \$ ${formatter.format(tradeValues.quantity?.formatted() ?? 0)}",
                                  style: const TextStyle(fontSize: 16)),
                              Text(amt.toString(),
                                  style: const TextStyle(fontSize: 16, color: Colors.grey))
                            ],
                          )),
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
                            if (fee != null && fee is PriorityFee)
                              Text("(${(fee as PriorityFee).priority})",
                                  style: const TextStyle(fontSize: 16))
                            else if (fee != null && fee is CustomFeeRate)
                              const Text("(Custom)", style: TextStyle(fontSize: 16))
                          ],
                        ),
                        FutureBuilder(
                            // TODO: Someone to remove all this Lightning stuff.
                            future: Future.value(1000),
                            builder: (BuildContext context, AsyncSnapshot<int> feeMsat) {
                              final msat = feeMsat.data ?? 0;

                              final Widget feeWidget;
                              if (msat < 1000 && msat > 0) {
                                feeWidget =
                                    Text("$msat msat", style: const TextStyle(fontSize: 16));
                              } else {
                                feeWidget = AmountText(
                                    amount: Amount((msat / 1000).round()),
                                    textStyle: const TextStyle(fontSize: 16));
                              }

                              return Column(crossAxisAlignment: CrossAxisAlignment.end, children: [
                                feeWidget,
                                if (fee != null && fee is PriorityFee)
                                  // TODO: estimate time for fixed fee
                                  Text((fee as PriorityFee).priority.toTimeEstimate(),
                                      style: const TextStyle(fontSize: 16, color: Colors.grey)),
                              ]);
                            })
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
                    var txid = await walletService.sendOnChainPayment(destination, amt, fee: fee);
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
