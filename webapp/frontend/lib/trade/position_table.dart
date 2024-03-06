import 'package:decimal/decimal.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/currency_change_notifier.dart';
import 'package:get_10101/common/direction.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/settings/channel_change_notifier.dart';
import 'package:get_10101/settings/dlc_channel.dart';
import 'package:get_10101/trade/close_position_confirmation_dialog.dart';
import 'package:get_10101/trade/position_change_notifier.dart';
import 'package:get_10101/trade/position_service.dart';
import 'package:get_10101/trade/quote_change_notifier.dart';
import 'package:get_10101/trade/quote_service.dart';
import 'package:intl/intl.dart';
import 'package:provider/provider.dart';
import 'package:collection/collection.dart';

class OpenPositionTable extends StatelessWidget {
  const OpenPositionTable({super.key});

  @override
  Widget build(BuildContext context) {
    final ChannelChangeNotifier changeNotifier = context.watch<ChannelChangeNotifier>();
    List<DlcChannel> channels = changeNotifier.getChannels() ?? [];
    DlcChannel? channel =
        channels.firstWhereOrNull((channel) => channel.channelState == ChannelState.signed);

    final positionChangeNotifier = context.watch<PositionChangeNotifier>();
    final positions = positionChangeNotifier.getPositions();

    final currencyChangeNotifier = context.watch<CurrencyChangeNotifier>();
    final currency = currencyChangeNotifier.currency;

    final quoteChangeNotifier = context.watch<QuoteChangeNotifier>();
    final quote = quoteChangeNotifier.getBestQuote();
    final Price midMarket =
        ((quote?.ask ?? Price.zero()) + (quote?.bid ?? Price.zero())) / Decimal.fromInt(2);

    if (positions == null) {
      return const Center(child: CircularProgressIndicator());
    }

    if (positions.isEmpty) {
      return const Center(child: Text('No data available'));
    } else {
      return buildTable(positions, quote, context, channel, midMarket, currency);
    }
  }

  Widget buildTable(List<Position> positions, BestQuote? bestQuote, BuildContext context,
      DlcChannel? channel, Price midMarket, Currency currency) {
    Widget actionReplacementLabel = createActionReplacementLabel(channel);
    return Table(
      border: const TableBorder(verticalInside: BorderSide(width: 0.5, color: Colors.black)),
      defaultVerticalAlignment: TableCellVerticalAlignment.middle,
      columnWidths: const {
        0: MinColumnWidth(FixedColumnWidth(100.0), FractionColumnWidth(0.1)),
        1: MinColumnWidth(FixedColumnWidth(100.0), FractionColumnWidth(0.1)),
        2: MinColumnWidth(FixedColumnWidth(100.0), FractionColumnWidth(0.1)),
        3: MinColumnWidth(FixedColumnWidth(150.0), FractionColumnWidth(0.1)),
        4: MinColumnWidth(FixedColumnWidth(100.0), FractionColumnWidth(0.1)),
        5: MinColumnWidth(FixedColumnWidth(100.0), FractionColumnWidth(0.1)),
        6: MinColumnWidth(FixedColumnWidth(200.0), FractionColumnWidth(0.25)),
        7: FixedColumnWidth(100),
      },
      children: [
        TableRow(
          decoration: BoxDecoration(
            color: tenTenOnePurple.shade300,
            border: const Border(bottom: BorderSide(width: 0.5, color: Colors.black)),
          ),
          children: [
            buildHeaderCell('Quantity'),
            buildHeaderCell('Entry Price'),
            buildHeaderCell('Liquidation Price'),
            buildHeaderCell('Margin'),
            buildHeaderCell('Leverage'),
            buildHeaderCell('Unrealized PnL'),
            buildHeaderCell('Expiry'),
            buildHeaderCell('Action'),
          ],
        ),
        for (var position in positions)
          TableRow(
            children: [
              buildTableCell(Text(position.direction == "Short"
                  ? "-${position.quantity}"
                  : "+${position.quantity}")),
              buildTableCell(Text(position.averageEntryPrice.toString())),
              buildTableCell(Text(position.liquidationPrice.toString())),
              buildAmountTableCell(position.collateral, currency, midMarket),
              buildTableCell(Text(position.leverage.formatted())),
              buildAmountTableCell(position.pnlSats, currency, midMarket),
              buildTableCell(
                  Text("${DateFormat('dd-MM-yyyy – HH:mm').format(position.expiry)} UTC")),
              buildTableCell(Center(
                child: SizedBox(
                  width: 100,
                  child: Visibility(
                    visible:
                        // don't show if the channel is already expired
                        position.expiry.isAfter(DateTime.now()) &&
                            channel != null &&
                            channel.channelState == ChannelState.signed &&
                            channel.subchannelState != null &&
                            channel.subchannelState == SubchannelState.established,
                    replacement: actionReplacementLabel,
                    child: ElevatedButton(
                        onPressed: () {
                          showDialog(
                              context: context,
                              builder: (BuildContext context) {
                                return TradeConfirmationDialog(
                                  direction: Direction.fromString(position.direction),
                                  onConfirmation: () {},
                                  bestQuote: bestQuote,
                                  pnl: position.pnlSats,
                                  fee: position.closingFee,
                                  payout: position.closingFee != null
                                      ? Amount(position.collateral.sats +
                                          (position.pnlSats?.sats ?? 0) -
                                          (position.closingFee?.sats ?? 0))
                                      : null,
                                  leverage: position.leverage,
                                  quantity: position.quantity,
                                );
                              });
                        },
                        child: const Text("Close", style: TextStyle(fontSize: 16))),
                  ),
                ),
              )),
            ],
          ),
      ],
    );
  }

  Widget createActionReplacementLabel(DlcChannel? channel) {
    Widget actionReplacementLabel = const SizedBox.shrink();
    if (channel != null && channel.subchannelState != null) {
      switch (channel.subchannelState) {
        case SubchannelState.established:
          actionReplacementLabel = Container(
              decoration: BoxDecoration(
                  color: Colors.green.shade300, borderRadius: BorderRadius.circular(15)),
              child: const Padding(
                padding: EdgeInsets.all(8.0),
                child: Center(
                    child: Text(
                  "Channel is active",
                )),
              ));
          break;
        case SubchannelState.settledOffered:
        case SubchannelState.settledReceived:
        case SubchannelState.settledAccepted:
        case SubchannelState.settledConfirmed:
        case SubchannelState.renewOffered:
        case SubchannelState.renewAccepted:
        case SubchannelState.renewConfirmed:
        case SubchannelState.renewFinalized:
          actionReplacementLabel = Container(
              decoration: BoxDecoration(
                  color: Colors.green.shade300, borderRadius: BorderRadius.circular(15)),
              child: const Padding(
                padding: EdgeInsets.all(8.0),
                child: Center(
                    child: Text(
                  "Pending",
                )),
              ));
          break;
        case SubchannelState.settled:
          actionReplacementLabel = Container(
              decoration: BoxDecoration(
                  color: Colors.green.shade300,
                  border: const Border(bottom: BorderSide(width: 0.5))),
              child: const Text(
                "Channel is active",
              ));
          break;
        case SubchannelState.closing:
        case SubchannelState.collaborativeCloseOffered:
          actionReplacementLabel = Container(
              decoration: BoxDecoration(
                  color: Colors.orange.shade300, borderRadius: BorderRadius.circular(15)),
              child: const Padding(
                padding: EdgeInsets.all(8.0),
                child: Center(
                    child: Text(
                  "Closing",
                )),
              ));
          break;
        case null:
        // nothing
      }
    }
    return actionReplacementLabel;
  }

  TableCell buildHeaderCell(String text) {
    return TableCell(
        child: Container(
            padding: const EdgeInsets.all(10),
            alignment: Alignment.center,
            child: Text(text,
                textAlign: TextAlign.center,
                style: const TextStyle(fontWeight: FontWeight.normal, color: Colors.white))));
  }

  TableCell buildTableCell(Widget child) => TableCell(
          child: SelectionArea(
        child: Center(
            child: Container(
                padding: const EdgeInsets.all(10), alignment: Alignment.center, child: child)),
      ));

  TableCell buildAmountTableCell(Amount? child, Currency currency, Price midMarket) {
    if (child == null) {
      return buildTableCell(const Text(""));
    }

    switch (currency) {
      case Currency.usd:
        return buildTableCell(Text(formatUsd(child * midMarket, decimalPlaces: 2)));
      case Currency.btc:
        return buildTableCell(Text(formatBtc(child)));
      case Currency.sats:
        return buildTableCell(Text(formatSats(child)));
    }
  }
}
