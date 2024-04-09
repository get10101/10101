import 'package:get_10101/common/model.dart';
import 'package:get_10101/services/quote_service.dart';

Amount calculateFee(Usd? quantity, BestQuote? quote, bool isLong) {
  if (quote?.fee == null || quote?.fee == 0 || quantity == null) {
    return Amount.zero();
  }

  return Amount(
      (calculateMargin(quantity, quote!, Leverage.one(), isLong).sats * quote.fee!).toInt());
}

Amount calculateMargin(Usd quantity, BestQuote quote, Leverage leverage, bool isLong) {
  if (isLong && quote.ask != null) {
    if (quote.ask!.asDouble == 0) {
      return Amount.zero();
    }
    return Amount.fromBtc(quantity.asDouble / (quote.ask!.asDouble * leverage.asDouble));
  } else if (!isLong && quote.bid != null) {
    if (quote.bid!.asDouble == 0) {
      return Amount.zero();
    }
    return Amount.fromBtc(quantity.asDouble / (quote.bid!.asDouble * leverage.asDouble));
  } else {
    return Amount.zero();
  }
}

Amount calculateLiquidationPrice(
    Usd quantity, BestQuote quote, Leverage leverage, double maintenanceMarginRate, bool isLong) {
  if (isLong && quote.bid != null) {
    return Amount((quote.bid!.asDouble * leverage.asDouble) ~/
        (leverage.asDouble + 1.0 - (maintenanceMarginRate * leverage.asDouble)));
  } else if (!isLong && quote.ask != null) {
    if (leverage.asDouble == 1.0) {
      return Amount(1048575);
    }

    return Amount((quote.ask!.asDouble * leverage.asDouble) ~/
        (leverage.asDouble - 1.0 + (maintenanceMarginRate * leverage.asDouble)));
  } else {
    return Amount.zero();
  }
}
