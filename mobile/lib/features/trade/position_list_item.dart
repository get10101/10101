import 'dart:async';

import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/fiat_text.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:intl/intl.dart';
import 'package:provider/provider.dart';

import 'contract_symbol_icon.dart';

class PositionListItem extends StatefulWidget {
  const PositionListItem({super.key, required this.position, required this.onClose});

  final Position? position;
  final Function onClose;

  @override
  State<PositionListItem> createState() => _PositionListItemState();
}

class _PositionListItemState extends State<PositionListItem> {
  bool isPositionExpired = false;

  @override
  void initState() {
    super.initState();
    if (widget.position != null) {
      Position notNullPosition = widget.position!;

      if (DateTime.now().toUtc().isAfter(notNullPosition.expiry)) {
        isPositionExpired = true;
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    if (widget.position == null) {
      return const NoPositionsListItem();
    }

    final formatter = NumberFormat();
    formatter.minimumFractionDigits = 2;
    formatter.maximumFractionDigits = 2;

    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;
    PositionChangeNotifier positionChangeNotifier = context.watch<PositionChangeNotifier>();

    // dart cannot promote...
    Position notNullPosition = widget.position!;

    // We're a bit conservative, we only enable action when we have both bid and ask
    bool priceAvailable = notNullPosition.direction == Direction.long
        ? positionChangeNotifier.price?.ask != null
        : positionChangeNotifier.price?.bid != null;

    if (!isPositionExpired) {
      Timer(notNullPosition.expiry.difference(DateTime.now().toUtc()), () {
        setState(() {
          isPositionExpired = true;
        });
      });
    }

    TextStyle dataRowStyle = const TextStyle(fontSize: 14);

    return Card(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(10.0, 10, 10, 0),
        child: Column(
          children: [
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    const ContractSymbolIcon(),
                    const SizedBox(
                      width: 10,
                    ),
                    Text(
                      notNullPosition.contractSymbol.label,
                      style: const TextStyle(fontWeight: FontWeight.bold),
                    ),
                    const SizedBox(
                      width: 5,
                    ),
                    Text(
                      notNullPosition.direction.keySuffix,
                      style: const TextStyle(fontWeight: FontWeight.bold),
                    ),
                  ],
                ),
              ],
            ),
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 10),
              child: Wrap(
                runSpacing: 5,
                children: [
                  ValueDataRow(
                      value: LoadingValue(
                          value: notNullPosition.unrealizedPnl,
                          builder: (pnl) {
                            return AmountText(
                                amount: pnl,
                                textStyle: dataRowStyle.apply(
                                    color: notNullPosition.unrealizedPnl!.sats.isNegative
                                        ? tradeTheme.loss
                                        : tradeTheme.profit));
                          }),
                      label: "Unrealized P/L"),
                  ValueDataRow(
                    value: AmountText(amount: notNullPosition.collateral),
                    label: "Margin",
                    valueTextStyle: dataRowStyle,
                    labelTextStyle: dataRowStyle,
                  ),
                  ValueDataRow(
                    value: Text(notNullPosition.leverage.formatted()),
                    label: "Leverage",
                    valueTextStyle: dataRowStyle,
                    labelTextStyle: dataRowStyle,
                  ),
                  ValueDataRow(
                    value: DateValue(notNullPosition.expiry),
                    label: "Expiry",
                    valueTextStyle: dataRowStyle,
                    labelTextStyle: dataRowStyle,
                  ),
                  ValueDataRow(
                    value: Text("${formatter.format(notNullPosition.quantity.toInt)} contracts",
                        style: dataRowStyle),
                    label: "Quantity",
                    labelTextStyle: dataRowStyle,
                  ),
                  ValueDataRow(
                    value: FiatText(amount: notNullPosition.liquidationPrice),
                    label: "Liquidation price",
                    valueTextStyle: dataRowStyle,
                    labelTextStyle: dataRowStyle,
                  ),
                  ValueDataRow(
                    value: FiatText(amount: notNullPosition.averageEntryPrice),
                    label: "Average entry price",
                    valueTextStyle: dataRowStyle,
                    labelTextStyle: dataRowStyle,
                  ),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.end,
                    children: [
                      ElevatedButton(
                        onPressed: notNullPosition.positionState == PositionState.closing ||
                                isPositionExpired ||
                                !priceAvailable
                            ? null
                            : () async {
                                await widget.onClose();
                              },
                        child: notNullPosition.positionState == PositionState.closing ||
                                isPositionExpired
                            ? const Row(
                                children: [
                                  SizedBox(
                                    width: 10,
                                    height: 10,
                                    child: CircularProgressIndicator(),
                                  ),
                                  Text("Closing ...")
                                ],
                              )
                            : const Text("Close Position"),
                      )
                    ],
                  )
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class NoPositionsListItem extends StatelessWidget {
  const NoPositionsListItem({super.key});

  @override
  Widget build(BuildContext context) {
    return const Card(
      child: ListTile(
        leading: ContractSymbolIcon(),
        title: Text("You don't have any position yet..."),
        subtitle: Text("Trade now to open a position!"),
      ),
    );
  }
}
