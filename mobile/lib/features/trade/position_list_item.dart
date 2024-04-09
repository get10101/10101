import 'dart:async';

import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/brag/brag.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:intl/intl.dart';
import 'package:provider/provider.dart';
import 'package:get_10101/features/trade/contract_symbol_icon.dart';

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
        ? positionChangeNotifier.bidPrice != null
        : positionChangeNotifier.askPrice != null;

    if (!isPositionExpired) {
      Timer(notNullPosition.expiry.difference(DateTime.now().toUtc()), () {
        setState(() {
          isPositionExpired = true;
        });
      });
    }

    TextStyle dataRowStyle = const TextStyle(fontSize: 14);

    Amount? unrealizedPnl = notNullPosition.unrealizedPnl;
    double pnlPercent =
        ((unrealizedPnl?.sats ?? Amount.zero().sats) / notNullPosition.collateral.sats) * 100.0;

    return Card(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(10.0, 10, 10, 0),
        child: Column(
          children: [
            Stack(
              children: [
                Row(
                  mainAxisAlignment: MainAxisAlignment.start,
                  crossAxisAlignment: CrossAxisAlignment.center,
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
                    const Spacer(),
                    ClipOval(
                      child: Material(
                        color: Colors.grey.shade100,
                        child: InkWell(
                          splashColor: tenTenOnePurple.shade200,
                          onTap: () {
                            showDialog(
                              context: context,
                              builder: (BuildContext context) {
                                return BragWidget(
                                  title: 'Share as image',
                                  onClose: () {
                                    Navigator.of(context).pop();
                                  },
                                  direction: notNullPosition.direction,
                                  leverage: notNullPosition.leverage,
                                  pnl: unrealizedPnl,
                                  pnlPercent: double.parse(pnlPercent.toStringAsFixed(0)).toInt(),
                                  entryPrice: Usd.fromDouble(notNullPosition.averageEntryPrice),
                                );
                              },
                            );
                          },
                          child: const SizedBox(
                              width: 32,
                              height: 32,
                              child: Icon(
                                FontAwesomeIcons.shareNodes,
                                size: 18,
                              )),
                        ),
                      ),
                    )
                  ],
                ),
              ],
            ),
            Padding(
              padding: const EdgeInsets.only(top: 5, bottom: 10),
              child: Wrap(
                runSpacing: 2,
                children: [
                  unrealizedPnl == null
                      ? const ValueDataRow(
                          type: ValueType.loading, value: "", label: "Unrealized P/L")
                      : Row(
                          mainAxisAlignment: MainAxisAlignment.spaceBetween,
                          children: [
                            Row(children: [
                              Text(
                                "Unrealized P/L",
                                style: dataRowStyle,
                              ),
                              const SizedBox(width: 2),
                              const Text("", style: TextStyle(fontSize: 12, color: Colors.grey)),
                            ]),
                            Row(
                              children: [
                                AmountText(
                                  amount: unrealizedPnl,
                                  textStyle: dataRowStyle.apply(
                                      color: unrealizedPnl.sats.isNegative
                                          ? tradeTheme.loss
                                          : tradeTheme.profit),
                                ),
                                Text(
                                  " (${pnlPercent.toStringAsFixed(2)}%)",
                                  style: dataRowStyle.apply(
                                      color: unrealizedPnl.sats.isNegative
                                          ? tradeTheme.loss
                                          : tradeTheme.profit),
                                ),
                              ],
                            )
                          ],
                        ),
                  ValueDataRow(
                    type: ValueType.amount,
                    value: notNullPosition.collateral,
                    label: "Margin",
                    valueTextStyle: dataRowStyle,
                    labelTextStyle: dataRowStyle,
                  ),
                  ValueDataRow(
                    type: ValueType.text,
                    value: notNullPosition.leverage.formatted(),
                    label: "Leverage",
                    valueTextStyle: dataRowStyle,
                    labelTextStyle: dataRowStyle,
                  ),
                  ValueDataRow(
                    type: ValueType.date,
                    value: notNullPosition.expiry,
                    label: "Expiry",
                    valueTextStyle: dataRowStyle,
                    labelTextStyle: dataRowStyle,
                  ),
                  ValueDataRow(
                    type: ValueType.contracts,
                    value: formatter.format(notNullPosition.quantity.toInt),
                    label: "Quantity",
                    valueTextStyle: dataRowStyle,
                    labelTextStyle: dataRowStyle,
                  ),
                  ValueDataRow(
                    type: ValueType.fiat,
                    value: notNullPosition.liquidationPrice,
                    label: "Liquidation price",
                    valueTextStyle: dataRowStyle,
                    labelTextStyle: dataRowStyle,
                  ),
                  ValueDataRow(
                    type: ValueType.fiat,
                    value: notNullPosition.averageEntryPrice,
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
                            : const Text(
                                "Close Position",
                                style: TextStyle(color: Colors.white),
                              ),
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
