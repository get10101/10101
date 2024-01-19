import 'package:bitcoin_icons/bitcoin_icons.dart';
import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/amount_text_input_form_field.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/common/theme.dart';

class TradeScreen extends StatefulWidget {
  static const route = "/trade";

  // TODO: get price from service
  final Quote quote = Quote(Price(41129.0), Price(41130.5));

  // for now just to avoid division by 0 errors, later we should introduce a maintenance margin
  final double maintenanceMargin = 0.001;

  TradeScreen({super.key});

  @override
  State<TradeScreen> createState() => _TradeScreenState();
}

class _TradeScreenState extends State<TradeScreen> {
  Quote? _quote;
  Usd? _quantity;
  Leverage _leverage = Leverage(1);
  bool isLong = true;

  final TextEditingController _marginController = TextEditingController();
  final TextEditingController _liquidationPriceController = TextEditingController();
  final TextEditingController _latestPriceController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _quote = widget.quote;
    _quantity = Usd(100);

    updateOrderValues();
  }

  @override
  Widget build(BuildContext context) {
    TenTenOneTheme theme = Theme.of(context).extension<TenTenOneTheme>()!;
    Color buyButtonColor = isLong ? theme.buy : theme.inactiveButtonColor;
    Color buyButtonBorderColor = theme.buy;
    Color sellButtonColor = isLong ? theme.inactiveButtonColor : theme.sell;
    Color sellButtonBorderColor = theme.sell;

    const spaceBetweenRows = SizedBox(height: 10);
    return Column(
      mainAxisAlignment: MainAxisAlignment.start,
      crossAxisAlignment: CrossAxisAlignment.center,
      children: <Widget>[
        SizedBox(
          width: 400,
          child: Column(
            children: [
              const SizedBox(height: 20),
              Row(
                crossAxisAlignment: CrossAxisAlignment.center,
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Expanded(
                    child: ElevatedButton(
                        onPressed: () => setState(() {
                              isLong = true;
                              updateOrderValues();
                            }),
                        style: ElevatedButton.styleFrom(
                            side: BorderSide(color: buyButtonBorderColor, width: 1),
                            backgroundColor: buyButtonColor,
                            minimumSize: const Size.fromHeight(50)),
                        child: const Text("Buy")),
                  ),
                  Expanded(
                    child: ElevatedButton(
                        onPressed: () => setState(() {
                              isLong = false;
                              updateOrderValues();
                            }),
                        style: ElevatedButton.styleFrom(
                            side: BorderSide(color: sellButtonBorderColor, width: 1),
                            backgroundColor: sellButtonColor,
                            minimumSize: const Size.fromHeight(50)),
                        child: const Text("Sell")),
                  ),
                ],
              ),
              const SizedBox(height: 20),
              Align(
                alignment: AlignmentDirectional.centerEnd,
                child: AmountInputField(
                  enabled: false,
                  label: isLong ? "Ask" : "Bid",
                  textAlign: TextAlign.right,
                  suffixIcon: const Icon(FontAwesomeIcons.dollarSign),
                  controller: _latestPriceController,
                ),
              ),
              const SizedBox(height: 20),
              Align(
                alignment: AlignmentDirectional.centerEnd,
                child: AmountInputField(
                  value: _quantity,
                  enabled: true,
                  label: "Quantity",
                  textAlign: TextAlign.right,
                  suffixIcon: const Icon(FontAwesomeIcons.dollarSign),
                  onChanged: (quantity) => setState(() {
                    _quantity = Usd.parseString(quantity);
                    updateOrderValues();
                  }),
                ),
              ),
              spaceBetweenRows,
              Align(
                alignment: AlignmentDirectional.centerEnd,
                child: AmountInputField(
                  value: _leverage,
                  enabled: true,
                  label: "Leverage",
                  textAlign: TextAlign.right,
                  onChanged: (leverage) => setState(() {
                    _leverage = Leverage(int.parse(leverage));
                    updateOrderValues();
                  }),
                ),
              ),
              spaceBetweenRows,
              Align(
                alignment: AlignmentDirectional.centerEnd,
                child: AmountInputField(
                  enabled: false,
                  label: "Margin",
                  suffixIcon: const Icon(BitcoinIcons.satoshi_v1),
                  textAlign: TextAlign.right,
                  controller: _marginController,
                ),
              ),
              spaceBetweenRows,
              Align(
                alignment: AlignmentDirectional.centerEnd,
                child: AmountInputField(
                  enabled: false,
                  label: "Liquidation",
                  suffixIcon: const Icon(FontAwesomeIcons.dollarSign),
                  textAlign: TextAlign.right,
                  controller: _liquidationPriceController,
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }

  void updateOrderValues() {
    if (_quantity != null && _quote != null) {
      _marginController.text = calculateMargin(_quantity!, _quote!, _leverage, isLong).formatted();
      _liquidationPriceController.text = calculateLiquidationPrice(
              _quantity!, _quote!, _leverage, widget.maintenanceMargin, isLong)
          .formatted();
    }

    if (isLong && _quote != null && _quote!.ask != null) {
      _latestPriceController.text = _quote!.ask!.formatted();
    } else if (!isLong && _quote != null && _quote!.bid != null) {
      _latestPriceController.text = _quote!.bid!.formatted();
    }
  }
}

Amount calculateMargin(Usd quantity, Quote quote, Leverage leverage, bool isLong) {
  if (isLong && quote.ask != null) {
    return Amount.fromBtc(quantity.asDouble / (quote.ask!.asDouble * leverage.asDouble));
  } else if (!isLong && quote.bid != null) {
    return Amount.fromBtc(quantity.asDouble / (quote.bid!.asDouble * leverage.asDouble));
  } else {
    return Amount.zero();
  }
}

Amount calculateLiquidationPrice(
    Usd quantity, Quote quote, Leverage leverage, double maintenanceMargin, bool isLong) {
  if (isLong && quote.ask != null) {
    return Amount((quote.bid!.asDouble * leverage.asDouble) ~/
        (leverage.asDouble + 1.0 + (maintenanceMargin * leverage.asDouble)));
  } else if (!isLong && quote.bid != null) {
    return Amount((quote.ask!.asDouble * leverage.asDouble) ~/
        (leverage.asDouble - 1.0 + (maintenanceMargin * leverage.asDouble)));
  } else {
    return Amount.zero();
  }
}
