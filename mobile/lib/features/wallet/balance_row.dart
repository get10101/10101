import 'package:flutter/material.dart';
import 'package:flutter_svg/svg.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/create_invoice_screen.dart';
import 'package:get_10101/features/wallet/send_screen.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_theme.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

import 'domain/payment_flow.dart';
import 'domain/wallet_type.dart';

class BalanceRow extends StatefulWidget {
  final WalletType walletType;
  final double iconSize;
  const BalanceRow({required this.walletType, this.iconSize = 30, super.key});

  @override
  State<BalanceRow> createState() => _BalanceRowState();
}

class _BalanceRowState extends State<BalanceRow> with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    duration: const Duration(milliseconds: 750),
    vsync: this,
  );

  bool _expanded = false;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    WalletTheme theme = Theme.of(context).extension<WalletTheme>()!;
    WalletChangeNotifier walletChangeNotifier = context.watch<WalletChangeNotifier>();
    TextStyle normal = const TextStyle(fontSize: 16.0);
    TextStyle bold = const TextStyle(fontWeight: FontWeight.bold, fontSize: 16.0);

    Amount amount;
    String name;
    Color rowBgColor;
    SvgPicture icon;

    if (widget.walletType == WalletType.lightning) {
      name = "Lightning";
      rowBgColor = theme.lightning;
      icon = SvgPicture.asset("assets/Lightning_logo.svg");
      amount = walletChangeNotifier.lightning();
    } else {
      name = "On-chain";
      rowBgColor = theme.onChain;
      icon = SvgPicture.asset("assets/Bitcoin_logo.svg");
      amount = walletChangeNotifier.onChain();
    }

    double balanceRowHeight = 40;
    double buttonSize = balanceRowHeight - 10;
    double buttonSpacing = 10;

    BalanceRowButton send = BalanceRowButton(
      flow: PaymentFlow.outbound,
      enabled: _expanded,
      buttonSize: buttonSize,
    );

    BalanceRowButton receive = BalanceRowButton(
      flow: PaymentFlow.inbound,
      enabled: _expanded,
      buttonSize: buttonSize,
    );

    double buttonWidth = send.width();

    return Padding(
      padding: const EdgeInsets.all(8.0),
      child: SizedBox(
        height: balanceRowHeight,
        child: Stack(
          alignment: Alignment.topLeft,
          children: [
            FadeTransition(
              opacity: _controller.drive(CurveTween(curve: Curves.easeIn)),
              child: Row(mainAxisAlignment: MainAxisAlignment.start, children: [
                send,
                SizedBox(
                  width: buttonSpacing,
                ),
                receive
              ]),
            ),
            PositionedTransition(
              rect: RelativeRectTween(
                      begin: RelativeRect.fill,
                      end: RelativeRect.fromLTRB(buttonWidth * 2 + buttonSpacing * 2, 0, 0, 0))
                  .animate(CurvedAnimation(parent: _controller, curve: Curves.easeOutBack)),
              child: GestureDetector(
                onTap: () {
                  _controller.stop();
                  setState(() => _expanded = !_expanded);

                  if (_expanded) {
                    _controller.forward();
                  } else {
                    _controller.reverse();
                  }
                },
                child: Container(
                  padding: const EdgeInsets.symmetric(horizontal: 4.0),
                  decoration: BoxDecoration(
                      gradient: LinearGradient(
                        begin: Alignment.centerLeft,
                        end: Alignment.centerRight,
                        transform: const GradientRotation(1.1),
                        stops: const [0, 0.5],
                        colors: [rowBgColor, theme.bgColor],
                      ),
                      border: Border.all(color: theme.borderColor),
                      borderRadius: const BorderRadius.all(Radius.circular(8))),
                  child: Row(children: [
                    Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 4.0),
                      child: SizedBox(height: widget.iconSize, width: widget.iconSize, child: icon),
                    ),
                    Expanded(child: Text(name, style: normal)),
                    AmountText(amount: amount, textStyle: bold),
                  ]),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class BalanceRowButton extends StatelessWidget {
  final PaymentFlow flow;
  final bool enabled;
  final double buttonSize;

  const BalanceRowButton(
      {super.key, required this.flow, required this.enabled, this.buttonSize = 40});

  double width() {
    // 2x padding from around the icon, 2x padding from inside the icon
    return buttonSize;
  }

  @override
  Widget build(BuildContext context) {
    IconData icon;
    if (flow == PaymentFlow.outbound) {
      icon = Icons.upload;
    } else {
      icon = Icons.download;
    }

    double buttonIconPadding = 5;

    return SizedBox(
      width: buttonSize,
      child: ElevatedButton(
        onPressed: !enabled
            ? null
            : () {
                if (flow == PaymentFlow.outbound) {
                  context.go(SendScreen.route);
                } else {
                  context.go(CreateInvoiceScreen.route);
                }
              },
        style: ElevatedButton.styleFrom(
          shape: const CircleBorder(),
          padding: EdgeInsets.all(buttonIconPadding),
        ),
        child: Center(
            child: Icon(
          icon,
          size: buttonSize - buttonIconPadding * 2,
        )),
        // tooltip: enabled ? "$action bitcoins on $type" : null,
        // padding: const EdgeInsets.all(horizontalPadding),
      ),
    );
  }
}
