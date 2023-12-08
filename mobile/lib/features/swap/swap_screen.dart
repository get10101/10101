import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/features/swap/swap_bottom_sheet.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

class SwapScreen extends StatefulWidget {
  static const label = "swap";

  const SwapScreen({Key? key}) : super(key: key);

  @override
  State<SwapScreen> createState() => _SwapScreenState();
}

class _SwapScreenState extends State<SwapScreen> {
  @override
  Widget build(BuildContext context) {
    PositionChangeNotifier positionChangeNotifier = context.watch<PositionChangeNotifier>();

    final position = positionChangeNotifier.positions[ContractSymbol.btcusd];

    List<Widget> widgets;

    final hasNonStablePosition = position != null && !positionChangeNotifier.hasStableUSD();

    if (hasNonStablePosition) {
      widgets = [
        Container(
          padding: const EdgeInsets.only(left: 10, right: 10, top: 20, bottom: 20),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Flexible(
                child: Column(
                  children: [
                    const Icon(FontAwesomeIcons.circleExclamation,
                        color: tenTenOnePurple, size: 60),
                    const SizedBox(height: 25),
                    const Text(
                        "We do not support having USD-P and a trade at the same time at the moment.\n\nPlease close your position to make use of USD-P.",
                        textAlign: TextAlign.justify,
                        style: TextStyle(color: Colors.black87, fontSize: 16)),
                    const SizedBox(height: 30),
                    SizedBox(
                      width: MediaQuery.of(context).size.width * 0.9,
                      child: ElevatedButton(
                          onPressed: () {
                            GoRouter.of(context).pop();
                            GoRouter.of(context).go(TradeScreen.route);
                          },
                          style: ButtonStyle(
                              padding:
                                  MaterialStateProperty.all<EdgeInsets>(const EdgeInsets.all(15)),
                              backgroundColor: MaterialStateProperty.resolveWith((states) {
                                return tenTenOnePurple;
                              }),
                              shape: MaterialStateProperty.resolveWith((states) {
                                return RoundedRectangleBorder(
                                    borderRadius: BorderRadius.circular(30.0),
                                    side: const BorderSide(color: tenTenOnePurple));
                              })),
                          child: const Text(
                            "Go to trade",
                            style: TextStyle(fontSize: 18, color: Colors.white),
                          )),
                    )
                  ],
                ),
              )
            ],
          ),
        )
      ];
    } else {
      widgets = [
        Padding(
          padding: EdgeInsets.only(bottom: MediaQuery.of(context).viewInsets.bottom),
          // the GestureDetector ensures that we can close the keyboard by tapping into the modal
          child: GestureDetector(
            onTap: () {
              FocusScopeNode currentFocus = FocusScope.of(context);

              if (!currentFocus.hasPrimaryFocus) {
                currentFocus.unfocus();
              }
            },
            child: SingleChildScrollView(child: SwapBottomSheet(position: position)),
          ),
        ),
      ];
    }

    return Column(
      mainAxisSize: MainAxisSize.min,
      children: widgets,
    );
  }
}

void showSwapDrawer(BuildContext context) {
  showModalBottomSheet<void>(
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(
          top: Radius.circular(20),
        ),
      ),
      clipBehavior: Clip.antiAlias,
      isScrollControlled: true,
      useRootNavigator: true,
      context: context,
      builder: (BuildContext context) => Container(
            decoration: const BoxDecoration(color: Colors.white),
            child: const Padding(
              padding: EdgeInsets.all(20),
              child: SwapScreen(),
            ),
          ));
}
