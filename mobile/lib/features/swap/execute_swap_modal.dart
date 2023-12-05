import 'dart:async';

import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

void showExecuteSwapModal(BuildContext context) {
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
        return const SingleChildScrollView(
            child: SizedBox(height: 300, child: Scaffold(body: ExecuteSwap())));
      });
}

class ExecuteSwap extends StatefulWidget {
  const ExecuteSwap({super.key});

  @override
  State<ExecuteSwap> createState() => _ExecuteSwapState();
}

class _ExecuteSwapState extends State<ExecuteSwap> {
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
    final pendingOrder = context.watch<SubmitOrderChangeNotifier>().pendingOrder;

    _timeout ??= Timer(const Duration(seconds: 30), () {
      setState(() => timeout = true);
    });

    Widget icon = Container();
    String text = "";

    switch (pendingOrder?.state) {
      case PendingOrderState.submissionFailed:
        {
          icon = Icon(FontAwesomeIcons.circleExclamation, color: Colors.red[600], size: 60);
          text = "Something went wrong";
        }
      case PendingOrderState.orderFailed:
        {
          icon = Icon(FontAwesomeIcons.circleExclamation, color: Colors.red[600], size: 60);
          text = "Something went wrong";
        }
      case PendingOrderState.orderFilled:
        {
          icon = const Icon(FontAwesomeIcons.solidCircleCheck, color: Colors.green, size: 60);
          text = "Your swap is complete";
          context.read<WalletChangeNotifier>().service.refreshLightningWallet();
        }
      default:
        {
          icon = const SizedBox(
            width: 60,
            height: 60,
            child: CircularProgressIndicator(color: tenTenOnePurple),
          );
          text = "Swapping";
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
                visible: timeout || PendingOrderState.orderFilled == pendingOrder?.state,
                child: SizedBox(
                  width: MediaQuery.of(context).size.width * 0.9,
                  child: ElevatedButton(
                      onPressed: () => GoRouter.of(context).pop(),
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
                        "Check balance",
                        style: TextStyle(fontSize: 18, color: Colors.white),
                      )),
                ))
          ],
        ),
      )),
    );
  }
}
