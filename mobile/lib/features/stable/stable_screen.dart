import 'package:flutter/material.dart';
import 'package:get_10101/common/fiat_text.dart';
import 'package:get_10101/features/stable/bitcoinize_confirmation_sheet.dart';
import 'package:get_10101/features/stable/stable_confirmation_sheet.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:provider/provider.dart';

import '../../common/value_data_row.dart';
import '../trade/domain/contract_symbol.dart';

class StableScreen extends StatefulWidget {
  static const route = "/stable";
  static const label = "Stable";

  const StableScreen({Key? key}) : super(key: key);

  @override
  State<StableScreen> createState() => _StableScreenState();
}

class _StableScreenState extends State<StableScreen> {
  @override
  Widget build(BuildContext context) {
    PositionChangeNotifier positionChangeNotifier = context.watch<PositionChangeNotifier>();

    final position = positionChangeNotifier.positions[ContractSymbol.btcusd];

    List<Widget> widgets;

    if (position == null) {
      widgets = [
        const Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Text("You don't have any synthetic USD yet!",
                style: TextStyle(color: Colors.grey, fontSize: 16))
          ],
        ),
        Expanded(
          child: Column(mainAxisAlignment: MainAxisAlignment.end, children: [
            ElevatedButton(
                onPressed: () => stableBottomSheet(context: context),
                child: const Text("Stabilize")),
            const SizedBox(height: 20)
          ]),
        )
      ];
    } else if (!positionChangeNotifier.hasStableUSD()) {
      widgets = [
        const Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Flexible(
              child: Text("Please close your current position before stabilizing your bitcoin!",
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(color: Colors.grey, fontSize: 16)),
            )
          ],
        )
      ];
    } else {
      widgets = [
        Column(children: [
          ValueDataRow(
            type: ValueType.date,
            value: position.expiry,
            valueTextStyle: const TextStyle(fontSize: 18),
            label: "Expiry",
            labelTextStyle: const TextStyle(fontSize: 18),
          ),
          const SizedBox(height: 10),
          ValueDataRow(
            type: ValueType.amount,
            value: position.getAmountWithUnrealizedPnl(),
            valueTextStyle: const TextStyle(fontSize: 18),
            label: "Sats",
            labelTextStyle: const TextStyle(fontSize: 18),
          )
        ]),
        Expanded(
          child: Column(mainAxisAlignment: MainAxisAlignment.end, children: [
            ElevatedButton(
                onPressed: () => bitcoinizeBottomSheet(context: context, position: position),
                child: const Text("Bitcoinize")),
            const SizedBox(height: 20)
          ]),
        )
      ];
    }

    List<Widget> children = [
      const SizedBox(height: 20),
      Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          FiatText(
            amount: positionChangeNotifier.getStableUSDAmountInFiat(),
            textStyle: const TextStyle(fontSize: 30, fontWeight: FontWeight.bold),
          )
        ],
      ),
      const SizedBox(height: 30),
    ];
    children.addAll(widgets);

    return Scaffold(
        body: Container(
            padding: const EdgeInsets.only(left: 15, right: 15),
            child: Column(
              children: children,
            )));
  }
}
