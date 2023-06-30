import 'dart:async';

import 'package:expandable/expandable.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:intl/intl.dart';

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

    // dart cannot promote...
    Position notNullPosition = widget.position!;

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
        padding: const EdgeInsets.all(10.0),
        child: ExpandablePanel(
          theme: const ExpandableThemeData(
            hasIcon: false,
            tapBodyToExpand: true,
            tapHeaderToExpand: true,
          ),
          header: Column(
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
                    ],
                  ),
                  Chip(
                    label: Text(notNullPosition.positionState.name),
                    backgroundColor: Colors.transparent,
                    shape: const StadiumBorder(side: BorderSide()),
                  )
                ],
              ),
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 10),
                child: Wrap(
                  runSpacing: 5,
                  children: [
                    notNullPosition.unrealizedPnl == null
                        ? const ValueDataRow(
                            type: ValueType.loading, value: "", label: "Unrealized P/L")
                        : ValueDataRow(
                            type: ValueType.amount,
                            value: notNullPosition.unrealizedPnl,
                            label: "Unrealized P/L",
                            valueTextStyle: dataRowStyle.apply(
                                color: notNullPosition.unrealizedPnl!.sats.isNegative
                                    ? tradeTheme.loss
                                    : tradeTheme.profit),
                            labelTextStyle: dataRowStyle,
                          ),
                    ValueDataRow(
                      type: ValueType.amount,
                      value: notNullPosition.collateral,
                      label: "Margin",
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
                      value: formatter.format(notNullPosition.quantity),
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
                  ],
                ),
              ),
            ],
          ),
          collapsed: Container(),
          expanded: Row(
            mainAxisAlignment: MainAxisAlignment.end,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              ElevatedButton(
                onPressed:
                    notNullPosition.positionState == PositionState.closing || isPositionExpired
                        ? null
                        : () async {
                            await widget.onClose();
                          },
                child: notNullPosition.positionState == PositionState.closing || isPositionExpired
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
              ),
            ],
          ),
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
