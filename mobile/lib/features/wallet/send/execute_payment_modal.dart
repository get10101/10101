import 'dart:async';

import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/wallet/domain/destination.dart';
import 'package:get_10101/features/wallet/send/payment_sent_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

void showExecuteUsdpPaymentModal(
    BuildContext context, Destination destination, Amount amount, bool payWithUsdp) {
  showModalBottomSheet<void>(
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(
          top: Radius.circular(20),
        ),
      ),
      clipBehavior: Clip.antiAlias,
      isScrollControlled: true,
      useRootNavigator: false,
      isDismissible: false,
      context: context,
      builder: (BuildContext context) {
        return SingleChildScrollView(
            child: SizedBox(
                height: 300,
                child: Scaffold(
                    body: ExecuteUsdpPayment(
                        amount: amount, destination: destination, payWithUsdp: payWithUsdp))));
      });
}

class ExecuteUsdpPayment extends StatefulWidget {
  final Destination destination;
  final Amount amount;
  final bool payWithUsdp;

  const ExecuteUsdpPayment(
      {super.key, required this.amount, required this.payWithUsdp, required this.destination});

  @override
  State<ExecuteUsdpPayment> createState() => _ExecuteUsdpPaymentState();
}

class _ExecuteUsdpPaymentState extends State<ExecuteUsdpPayment> {
  Timer? _timeout;
  bool timeout = false;
  bool sent = false;

  @override
  void dispose() {
    _timeout?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final walletService = context.read<WalletChangeNotifier>().service;
    final paymentChangeNotifier = context.watch<PaymentChangeNotifier>();
    final pendingOrder = context.watch<SubmitOrderChangeNotifier>().pendingOrder;

    _timeout ??= Timer(const Duration(seconds: 30), () {
      setState(() => timeout = true);
    });

    Widget icon = const SizedBox(
      width: 60,
      height: 60,
      child: CircularProgressIndicator(color: tenTenOnePurple),
    );
    String text = "";

    if ((pendingOrder?.state == PendingOrderState.orderFilled || !widget.payWithUsdp) && !sent) {
      if (widget.payWithUsdp) {
        logger.d("Order has been filled, attempting to send payment");
      }
      try {
        walletService.sendOnChainPayment(widget.destination, widget.amount);
        setState(() => sent = true);
      } catch (error) {
        logger.e("Failed to send payment: $error");
        context.read<PaymentChangeNotifier>().failPayment();
      }
    }

    switch (paymentChangeNotifier.getPaymentStatus()) {
      case PaymentStatus.pending:
        {
          if (pendingOrder?.state != PendingOrderState.orderFilled && widget.payWithUsdp) {
            text = "Swapping to sats";
          } else {
            text = "Sending payment";
          }
        }
      case PaymentStatus.success:
        {
          icon = const Icon(FontAwesomeIcons.solidCircleCheck, color: Colors.green, size: 60);
          text = "Sent";
        }
      case PaymentStatus.failed:
        {
          icon = Icon(FontAwesomeIcons.circleExclamation, color: Colors.red[600], size: 60);
          text = "Something went wrong";
        }
    }

    return Container(
      color: Colors.white,
      padding: const EdgeInsets.all(20),
      child: SafeArea(
          child: Center(
        child: Column(
          children: [
            const SizedBox(height: 25),
            icon,
            const SizedBox(height: 25),
            Text(text, style: const TextStyle(fontSize: 22)),
            const Spacer(),
            Visibility(
                visible: timeout ||
                    [PaymentStatus.success, PaymentStatus.failed]
                        .contains(paymentChangeNotifier.getPaymentStatus()),
                child: SizedBox(
                  width: MediaQuery.of(context).size.width * 0.9,
                  child: ElevatedButton(
                      onPressed: () => GoRouter.of(context).go(WalletScreen.route),
                      style: ButtonStyle(
                          padding: MaterialStateProperty.all<EdgeInsets>(const EdgeInsets.all(15)),
                          backgroundColor: MaterialStateProperty.resolveWith((states) {
                            return tenTenOnePurple;
                          }),
                          shape: MaterialStateProperty.resolveWith((states) {
                            return RoundedRectangleBorder(
                                borderRadius: BorderRadius.circular(30.0),
                                side: const BorderSide(color: tenTenOnePurple));
                          })),
                      child: const Text(
                        "Done",
                        style: TextStyle(fontSize: 18, color: Colors.white),
                      )),
                ))
          ],
        ),
      )),
    );
  }
}
