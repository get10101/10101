import 'package:flutter/material.dart';
import 'package:flutter_svg/svg.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/receive_screen.dart';
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

    return Padding(
      padding: const EdgeInsets.all(8.0),
      child: Stack(
        alignment: Alignment.center,
        children: [
          FadeTransition(
            opacity: _controller.drive(CurveTween(curve: Curves.easeIn)),
            child: Row(children: [
              BalanceRowButton(
                walletType: widget.walletType,
                flow: PaymentFlow.outbound,
                enabled: _expanded,
              ),
              BalanceRowButton(
                walletType: widget.walletType,
                flow: PaymentFlow.outbound,
                enabled: _expanded,
              ),
            ]),
          ),
          PositionedTransition(
            rect: RelativeRectTween(
                    begin: RelativeRect.fill,
                    end: RelativeRect.fromLTRB(
                        BalanceRowButton.width(context) * 2 + 4.0 + 8.0, 0, 0, 0))
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
    );
  }
}

class BalanceRowButton extends StatelessWidget {
  final WalletType walletType;
  final PaymentFlow flow;
  final bool enabled;
  static const double horizontalPadding = 8;

  const BalanceRowButton(
      {super.key, required this.walletType, required this.flow, required this.enabled});

  static double width(BuildContext context) {
    // 2x padding from around the icon, 2x padding from inside the icon
    return (horizontalPadding * 4) + (IconTheme.of(context).size ?? 24.0);
  }

  @override
  Widget build(BuildContext context) {
    WalletTheme theme = Theme.of(context).extension<WalletTheme>()!;

    String action;
    IconData icon;
    if (flow == PaymentFlow.outbound) {
      icon = Icons.upload;
      action = "Send";
    } else {
      icon = Icons.download;
      action = "Receive";
    }

    String type;
    if (walletType == WalletType.lightning) {
      type = "lightning";
    } else {
      type = "chain";
    }

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: horizontalPadding),
      child: IconButton(
        onPressed: !enabled
            ? null
            : () {
                if (flow == PaymentFlow.outbound) {
                  context.go(SendScreen.route);
                } else {
                  context.go(ReceiveScreen.route);
                }
              },
        tooltip: enabled ? "$action bitcoins on $type" : null,
        icon: Icon(icon),
        style: theme.iconButtonStyle,
        padding: const EdgeInsets.all(horizontalPadding),
      ),
    );
  }
}
