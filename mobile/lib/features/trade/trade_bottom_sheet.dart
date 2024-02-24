import 'package:flutter/material.dart';
import 'package:get_10101/common/modal_bottom_sheet_info.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/trade_bottom_sheet_tab.dart';
import 'package:get_10101/features/trade/trade_tabs.dart';
import 'package:get_10101/util/constants.dart';

tradeBottomSheet({required BuildContext context, required Direction direction}) {
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
    builder: (BuildContext context) {
      return SafeArea(
        child: Padding(
          padding: EdgeInsets.only(bottom: MediaQuery.of(context).viewInsets.bottom),
          // the GestureDetector ensures that we can close the keyboard by tapping into the modal
          child: GestureDetector(
            onTap: () {
              FocusScopeNode currentFocus = FocusScope.of(context);

              if (!currentFocus.hasPrimaryFocus) {
                currentFocus.unfocus();
              }
            },
            child: SingleChildScrollView(
              child: SizedBox(
                  // TODO: Find a way to make height dynamic depending on the children size
                  // This is needed because otherwise the keyboard does not push the sheet up correctly
                  height: 470,
                  child: TradeBottomSheet(direction: direction)),
            ),
          ),
        ),
      );
    },
  );
}

class TradeBottomSheet extends StatelessWidget {
  final Direction direction;

  const TradeBottomSheet({required this.direction, super.key});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(20),
      child: TradeTabs(
        tabBarPadding: const EdgeInsets.only(bottom: 10.0),
        tabs: const ["Buy", "Sell"],
        keys: const [tradeScreenBottomSheetTabsBuy, tradeScreenBottomSheetTabsSell],
        tabBarViewChildren: const [
          TradeBottomSheetTab(
            direction: Direction.long,
            buttonKey: tradeScreenBottomSheetButtonBuy,
          ),
          TradeBottomSheetTab(
            direction: Direction.short,
            buttonKey: tradeScreenBottomSheetButtonSell,
          ),
        ],
        selectedIndex: direction == Direction.long ? 0 : 1,
        topRightWidget: const Row(
          children: [
            Text(
              "Market Order",
              style: TextStyle(color: Colors.grey),
            ),
            ModalBottomSheetInfo(
                closeButtonText: "Back to order",
                child: Text("While in beta only market orders are enabled in the 10101 app.\n\n"
                    "Market orders are executed at the best market price. \n\nPlease note that the displayed "
                    "price is the best market price at the time but due to fast market "
                    "movements the market price for order fulfillment can be slightly different."))
          ],
        ),
      ),
    );
  }
}
